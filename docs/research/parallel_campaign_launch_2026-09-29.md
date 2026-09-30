# Parallel five-loop campaign: owner-operated launch

Current consolidated instructions: [September30 Ready/Union delivery](consolidated_campaign_launch_2026-09-30.md).
The old Epoch/W50 command below is retained only as historical evidence and
must not be mistaken for the current recommended recipe.

**September30: superseded experimental recipe, not the final launch handoff.**
The user now plans200 physical cores and will pause LC2 before launch. A bounded
Epoch scheduling repair and further ordering/deepening checks are in progress.
Preserve this50-worker recipe as evidence/rollback only; final tested build and
resource instructions will be supplied separately. No agent operates LC2.

**Status: compiled, tested candidate for an owner-operated experimental run;
not qualified as a faster replacement.** The measured controls and limitations
are in [the delivery report](parallel_epoch_delivery_2026-09-29.md).
Only the owner starts or stops production. LC2 continues unchanged alongside
the new campaign.

Consolidation note (September30,01:07 UTC): final monitored `3428b519` passes
lifecycle, all individual/combined four-loop and representative finite/hot
five-loop cold controls. Reindexed four-loop inputs give mixed-sign near-parity
across two pairs; W50 four-loop correctness also passes. However, the original
combined comparison is11.41% slower than Ready and the finite/hot five-loop
comparisons are25.57%/20.50% slower at their matched W16/W12 budgets. W50
five-loop performance is unmeasured. Do not launch expecting a demonstrated
speedup or guaranteed completion. Keeping LC2 alone is a reasonable choice.

The requested coloured CPU/balance dashboard and dual-axis plot at `adcee12c`
are frozen separately. The additional discovery-strategy API is in an isolated
native-test build and is **not** present in this executable; it is not needed
to apply the existing saved programs in this campaign. No native regeneration
or recompile is required to use the candidate below.

## Frozen executable and environment

The optimized executable is already built on this machine; another compilation
is unnecessary. Its native source is the clean workspace-local clone at `3428b519`:

```text
source: /common/dev/rustred/TMP/codex-parallel-campaign.oiPK29/repo
binary: /common/dev/rustred/TMP/codex-parallel-campaign.oiPK29/candidate-bin/rustred-3428b519
SHA256: 321b02b166c61dae927a220b7b8007b4659fef009d2b5b003084830b0f43eca3
profile: campaign, opt3, fat LTO, one codegen unit
Python steering/dashboard: /common/dev/rustred/TMP/codex-parallel-campaign.oiPK29/python-delivery-adcee12c
```

The Python revision improves monitoring only; it does not change the frozen
native engine or selected programs. Symbolica is clean `ef0db494`. Keep the
existing `SYMBOLICA_LICENSE` exported;
no license value belongs in a committed command or file.

Open a new tab from your existing Zellij session, or run:

```bash
zellij --session rustred action new-tab --name codex_astra \
  --cwd /common/dev/rustred/TMP/codex-parallel-campaign.oiPK29/python-delivery-adcee12c
```

In that new tab:

```bash
cd /common/dev/rustred/TMP/codex-parallel-campaign.oiPK29/python-delivery-adcee12c
nix develop
```

For an optional rebuild, first enter the **native source** directory shown
above, not the separate Python worktree. The tested build command is
`cargo build --profile campaign --locked --offline -j8 -p rustred-app --bin rustred`.
The delivered immutable binary is preferred for reproducing these measurements.
Family inputs, query order and the exposed runtime policy choices do not require
a Rust rebuild.

## Prepare once, retaining the original input scope

This copies the existing67 owner payloads without regenerating IBPs. It retains
the exact183 ordered query objects and declares all116 physical/convenience
queries required, with the67 original helpers auxiliary. It does not import
LC2's checkpoint or progress: Epoch uses a fresh CP6 campaign.

The destination must not already exist. If it does, inspect it and choose a new
name; never delete an existing campaign just to make this command succeed.

```bash
STAGE_B_TREE=/common/dev/rustred/TMP/codex-parallel-campaign.oiPK29/python-delivery-adcee12c
STAGE_B_PYTHON=/nix/store/2dkfxh789byan1h81sjhjzsijjsfb57m-python3-3.11.15-env/bin/python
STAGE_B_BINARY=/common/dev/rustred/TMP/codex-parallel-campaign.oiPK29/candidate-bin/rustred-3428b519
STAGE_B_CAMPAIGN=/common/dev/rustred/campaigns/five-loop-qcd-feynman-d9d10-epoch-rolling-union

"$STAGE_B_PYTHON" -B "$STAGE_B_TREE/examples/python/production_saved_owner_campaign.py" \
  --campaign-directory "$STAGE_B_CAMPAIGN" \
  --prepare-from /common/dev/rustred/campaigns/five-loop-qcd-feynman-d9d10-lc2 \
  --queries "$STAGE_B_TREE/examples/input/five_loop_qcd_feynman_d9d10/queries.json" \
  --attach "$STAGE_B_TREE/examples/input/five_loop_qcd_feynman_d9d10/entry-plan-receipt.json" \
  --query-order preserve --executable "$STAGE_B_BINARY" \
  --workers 50 --cpus 32-81 \
  --publication-policy epoch --epoch-inspector-lookup snapshot \
  --epoch-rolling --epoch-dispatch fifo \
  --transfer-unreserved-lookahead 256 --g2-residual-anchors union \
  --frontier-policy stop --auto-rescue --max-rescues 32 \
  --checkpoint-interval-seconds 14400 \
  --max-memory-bytes 400000000000 --ram-guard-margin-percent 5 \
  --host-memory-reserve-bytes 150000000000 \
  --swap-growth-stop-bytes-per-second 33554432 --swap-growth-stop-seconds 120 \
  --json
```

This **writes** the new campaign, frozen executable and steering, but does not
start the solver. `--json` is formatting, not a read-only dry run. The final plan
must show the stated policy, query roles, executable hash and unchanged owner
payload identities. The query SHA256 is
`42a0c62771b6e7c53cc937d46ad9505e33c282db31a8d6f64846ecbca749ef64`.

The50 physical cores32–81 do not overlap LC2's128–227. The400GB ceiling and150GB
host floor are decimal bytes, not a memory reservation. The supervisor admits
at most `min(400GB, available − 150GB)` and requests save/stop at95% of that
effective ceiling or at the live host-memory floor. This is intended to make
the new campaign yield before LC2's unchanged50GB floor; cooperative drain is
not an instantaneous hard-memory guarantee. LC2's600GB policy is not changed.

## Start, stop and resume

If you choose the experimental alongside run with these limitations, start in
`codex_astra`:

```bash
"$STAGE_B_PYTHON" -B "$STAGE_B_TREE/examples/python/production_saved_owner_campaign.py" \
  --campaign-directory "$STAGE_B_CAMPAIGN" --start
```

Use one ordinary Ctrl-C for a cooperative checkpoint and wait for its saved
message and worker drain. Later, with the same variables and frozen campaign:

```bash
"$STAGE_B_PYTHON" -B "$STAGE_B_TREE/examples/python/production_saved_owner_campaign.py" \
  --campaign-directory "$STAGE_B_CAMPAIGN" --resume --start
```

There is no CP5-to-CP6 conversion and no executable-upgrade path for this CP6
launcher. Keep LC2 and the new campaign independent. Rollback means saving and
stopping only the new campaign; LC2 remains running. Do not overwrite either
campaign's binary, input bindings, checkpoint or amendment chain.

FIFO is the baseline. Adaptive changes pending-job dispatch, not algebraic
pivots; it is a separate fresh-campaign option, not a later resume toggle. No
ordering choice is claimed optimal or guaranteed to finish five loops.

The frozen Python tree includes the aligned coloured terminal dashboard and
the presentation-independent `telemetry.jsonl` stream. Observed/total cores,
pending growth per completion and the dimensionless closure balance use the
requested threshold colours. `NO_COLOR=1` disables colours. To plot a chosen
run's saved observations without touching the solver:

```bash
"$STAGE_B_PYTHON" -B "$STAGE_B_TREE/examples/python/plot_campaign_rates.py" \
  "$STAGE_B_CAMPAIGN/runs/RUN_ID/telemetry.jsonl" \
  --output "$STAGE_B_CAMPAIGN/runs/RUN_ID/domains-and-net-rate.svg"
```

Replace `RUN_ID` with the actual run directory. The left axis is the unresolved
domain total and the right axis is the raw discovery-minus-closure rate in
domains/second. The plotted rate is deliberately not the normalized dashboard
balance. Closure counts are scan-batched; neither view supplies an ETA.

Automatic rescue is bounded to recognized cases and append-only amendments;
unknown failures stop explicitly. Required queries cannot be relabelled or
discarded. Queue exhaustion and exit4/checkpoint-only summaries are not a
closure certificate. Final scoped acceptance requires cold reinspection of all
116 required queries and their live dependencies, using raw checkpoint input,
`--no-result`, `--reinspect all`, `--reference-levers off`,
`--certification-scope physics-queries` and `--require-closure`. Abandoned
auxiliary helpers are not silently counted as closed.
