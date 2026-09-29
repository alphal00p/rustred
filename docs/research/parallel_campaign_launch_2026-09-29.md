# Parallel five-loop campaign: owner-operated launch

**Status: commands independently audited; performance qualification still in
progress. Do not launch from this draft yet.** The final decision and measured
controls belong in [the delivery report](parallel_epoch_delivery_2026-09-29.md).
Only the owner starts or stops production. LC2 continues unchanged alongside
the new campaign.

## Frozen executable and environment

The optimized executable is already built on this machine; another compilation
is unnecessary. Its source is the clean workspace-local clone at `e1bdb9e7`:

```text
source: /common/dev/rustred/TMP/codex-parallel-campaign.oiPK29/repo
binary: /common/dev/rustred/TMP/codex-parallel-campaign.oiPK29/candidate-bin/rustred-e1bdb9e7
SHA256: b5bd346cd9bfa925a4324031660cb3b2993e23c11f1e53765cb6758b87d9d95c
profile: campaign, opt3, fat LTO, one codegen unit
```

The feature branch subsequently merges the colleague's citation and Symbolica
compatibility update. The locked Symbolica checkout and selected campaign
engine sources are unchanged. Keep the existing `SYMBOLICA_LICENSE` exported;
no license value belongs in a committed command or file.

Open a new tab from your existing Zellij session, or run:

```bash
zellij --session rustred action new-tab --name codex_astra \
  --cwd /common/dev/rustred/TMP/codex-parallel-campaign.oiPK29/repo
```

In that new tab:

```bash
cd /common/dev/rustred/TMP/codex-parallel-campaign.oiPK29/repo
nix develop
```

For an optional rebuild of this exact source, the tested build command is
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
STAGE_B_TREE=/common/dev/rustred/TMP/codex-parallel-campaign.oiPK29/repo
STAGE_B_PYTHON=/nix/store/2dkfxh789byan1h81sjhjzsijjsfb57m-python3-3.11.15-env/bin/python
STAGE_B_BINARY=/common/dev/rustred/TMP/codex-parallel-campaign.oiPK29/candidate-bin/rustred-e1bdb9e7
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

After the final delivery report qualifies the build, start in `codex_astra`:

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

Automatic rescue is bounded to recognized cases and append-only amendments;
unknown failures stop explicitly. Required queries cannot be relabelled or
discarded. Queue exhaustion and exit4/checkpoint-only summaries are not a
closure certificate. Final scoped acceptance requires cold reinspection of all
116 required queries and their live dependencies, using raw checkpoint input,
`--no-result`, `--reinspect all`, `--reference-levers off`,
`--certification-scope physics-queries` and `--require-closure`. Abandoned
auxiliary helpers are not silently counted as closed.
