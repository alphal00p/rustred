# Restarting a saved-rule campaign at a lower input rank

`examples/python/prepare_rank_campaign.py` prepares a **new** campaign using
existing saved rules. It neither generates IBPs nor starts a solver.

```bash
cd /common/dev/rustred
nix develop /common/dev/rustred --command python -B \
  examples/python/prepare_rank_campaign.py \
  --source-campaign campaigns/five-loop-a1-banana485-20261004 \
  --destination campaigns/my-five-loop-rank0 \
  --max-numerator-rank 0
```

The destination must not exist. The original campaign and its checkpoints are
not modified. Ordinary `nix develop /common/dev/rustred` uses the tracked flake;
do not use `path:/common/dev/rustred`, which copies ignored campaign data too.

## What rank zero means

Each original required starting query is intersected with numerator rank zero.
All its other coordinate and positive-propagator-power restrictions remain
unchanged. Empty intersections stay in the input for native admission to
handle. This is the scalar part of the frozen physical request, **not** a new
claim covering arbitrary positive propagator powers.

By default auxiliary starting requests are omitted; `--include-auxiliary`
retains them with the same rank cap. All saved owner rules, routes and repair
overlays are retained in either mode. In particular, omitting auxiliary
starting requests does not remove those owners' usable reduction rules.
Some auxiliary requests have unbounded positive powers even at rank zero.

The rank limit applies only to starting inputs. Successors required to reduce
them are not clipped: reduction may encounter domains with higher or genuinely
unbounded numerator rank. The dashboard reports these encountered bounds.
Rank here is the sum of powers of inactive scalar products, not twice that
number (their Lorentz numerator degree).

The preparation receipt records the original and restricted query hashes,
retained/removed IDs, and changed caps. It makes no closure claim. Closure
requires discharging the resulting reachable obligations in the native engine.

## Freeze and launch

Build the native CLI once, then use the normal production launcher to freeze
the executable and resource policy into the new campaign. For example:

```bash
nix develop /common/dev/rustred --command \
  cargo build --release --locked -p rustred-app --bin rustred
nix develop /common/dev/rustred --command python -B \
  examples/python/production_saved_owner_campaign.py \
  --campaign-directory campaigns/my-five-loop-rank0 \
  --executable target/release/rustred \
  --workers 32 --cpus 64-95 \
  --max-memory-bytes 600000000000 \
  --host-memory-reserve-bytes 150000000000 \
  --publication-policy epoch --epoch-inspector-lookup snapshot \
  --epoch-rolling --epoch-dispatch fifo \
  --epoch-publication-order oldest-prefix \
  --epoch-cut-size 16 --epoch-window 76 \
  --inspection-workers 31 --g2-residual-anchors union \
  --transfer-unreserved-lookahead 256 \
  --checkpoint-interval-seconds 3600 \
  --frontier-policy stop --no-auto-rescue
```

The second command prepares and displays the native invocation but does not
launch it. Start manually with the same launcher and campaign directory plus
`--start`. After a clean interruption, use `--resume --start` on that same
campaign. Ctrl+C requests an orderly checkpoint save; wait for its completion.
The memory guard may lower the admitted memory budget to preserve host headroom.

For rank1, rank2, etc., create another fresh destination with the corresponding
`--max-numerator-rank`. Always derive it from the **original** saved-rule
campaign, not the rank-zero stage. The preparer refuses the latter because it
would retain the earlier cap. Saved rules are reused, but a lower-rank
checkpoint is not silently widened into a higher-rank campaign.

## Reading the corrected monitor

- `Completed-scan D/C` is new discoveries divided by new recursive closures
  between the **last two completed scans**, with the elapsed interval and
  scan age. It remains visible when stale; it is not a closure ETA. The
  trailing-hour observation is shown separately.
- `Next closure scan` reports throttle eligibility. Once due, the coordinator
  must reach a monitoring boundary. It is not a promise of an exact start time.
- `Checkpoint size` is the last saved generation's referenced payload bytes,
  including sealed record segments, not the size of all old files in its folder.
- `Max scheduled rank` is a bound derived from every admitted domain's geometry,
  including restored domains. Truly unbounded domains are labelled as such;
  absence of an explicit rank cap alone does not imply infinity.

New builds read pre-update checkpoints, but old binaries do not understand the
new persisted scan-history field. Keep a pre-update checkpoint for rollback
to an old executable. New rank stages use fresh checkpoints regardless.
