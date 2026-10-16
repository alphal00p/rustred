# User-controlled finite campaign limits and dashboard

## Next extension: chase monitoring and 150 workers

The follow-up adds5/30-minute gap convergence and current-chase elapsed time,
removes the fixed64-worker ceiling in favor of actual CPU/license admission,
and permits a Python `--workers` override without editing frozen policy files.
It also skips redundant resident-baseline probes on known live duplicates and
buffers the event journal while flushing every complete record. These changes
do not alter the admitted scope, rules, terminal convention or checkpoint schema.

After the current rank4 run **and its publication** have finished, select the
new optimized executable explicitly for the next extension:

```bash
cd /shared/codex-2/rustred
export RUSTRED_CAMPAIGN="$PWD/campaigns/five-loop-finite-r0-d10-resident-fixed-20261016"
nix develop --command python -B examples/python/saved_campaign.py extend \
  --campaign "$RUSTRED_CAMPAIGN" --rank 5 \
  --executable "$PWD/TMP/releases/20261016-finite-convergence-150/rustred" \
  --workers 150 --cpus 0-63,96-181 \
  --max-memory-bytes 600000000000 --memory-policy process-limit
```

This command retains D≤10 and prior completed work. It freezes/selects the new
executable at that future launch; do not overwrite the running campaign's binary
or change its current steering. Subsequent extensions retain the selected build
and150-worker policy. No `--resume` is needed with `extend`; after an interrupted
stage, use `run --resume` without appending a second rank stage.

On this host the selected CPU IDs are150 distinct physical cores and avoid the
older campaign's64–95 reservation. This is not an exclusive host reservation or
a promise of proportional speedup; shared membership, memory bandwidth and serial
commit work can limit scaling. Full validation and executable identity are in
the latest `CODEX_PROGRESS.md` delivery entry.

## What changed

The earlier R2 run stopped at its configured4096 immutable Work segments:
generation4129,314/348 required rows closed,36,994,385 completed Work records.
Its committed prefix was intact. This was a storage-policy refusal, not a
mathematical frontier or an exhausted RAM allowance.

Finite resource policy now follows the complete path: source admission,
tracing, publication, refinement, inspection, runtime export and cold loading.
Readers no longer silently clamp that policy to4096 segments,1024 amendments,
or their former smaller JSON transport limits. Reader grants are explicit
caller settings, never adopted from an untrusted artifact.

- Aggregate count and IO stops accept `null` for no requested cap.
- Explicit numeric caps remain enforced; changing them does not widen the
  mathematical input scope or change the window partition.
- Cache/page sizing and frame/transient allocation settings remain numeric.
- The former64MiB native frame ceiling is configurable. Bincode still checks
  allocation claims before allocating and rejects malformed/trailing data.
- The process-tree RAM guard and checked integer arithmetic remain active.
- No automatic checkpoint compaction was added. Removing a cap does not bound
  total disk use, establish completion time or prove unrestricted closure.

The high-memory policy keeps32 workers and the optional300GB resident Work
index. It removes cumulative run/page/file/disk/source/census caps and allows
1GiB manifest/state frames with2GiB transient decode headroom. Those grants
do not preallocate their maximum sizes. Users can select smaller numeric limits.

The subsequent resident-accounting repair replaces the configured directory
cap with the actual retained directory capacity. An uncapped (`null`) directory
grant had been interpreted as live storage, causing an overflow during initial
resident hydration. The repaired calculation keeps decode headroom explicit,
counts shared storage once, and accounts for old/new directory coexistence
during commits. It remains a conservative payload estimate, not exact RSS.

New finite launches default to `--memory-policy process-limit`. A requested
600 GB ceiling stays 600 GB, with a 570 GB soft threshold at the default 5%
margin. Host available RAM is reported as telemetry; it does not clamp this
limit. `--memory-policy host-aware` explicitly opts into admission that can
reduce the limit using host availability and the requested reserve.

The finite solve now has a bordered, colored Ratatui table with preparation
counts, committed required-row progress, checkpoint/window details, measured
CPU cores versus reserved workers, memory and rates. Input progress shows
processed/total occurrences, including overlaps across requests. Its total is
summed once from cached native admission counts; completed-window counts and
last-window new Work/reuse hits use existing committed metadata. No heartbeat
recounts inputs, scans the legacy closure ledger or derives an ETA. Process RSS,
the requested cap/soft threshold and host available/total RAM use decimal GB.
Window changes reset
window-local rate sampling. Startup is a separate colored phase/elapsed line;
publication retains its existing table. Error output follows the table rather
than overwriting it. Non-TTY operation uses bounded periodic progress lines.

## Validation and limits of the evidence

The resident repair passed328 targeted native tests (four ignored fixtures),
one separate public resident-budget test,217 Python tests and three real-native
PTY checks. The latter exercise cancellation, correct19-row terminal cleanup,
the requested600GB display and separate host-memory telemetry. The complete
and cancelled terminal screens were visually reviewed.

The full release R0 five-loop scratch run with the **unchanged shipped profile**
completed116/116 required rows,2557 input occurrences,300673 Work and81 windows.
Solve plus preparation took150.404s with7.7085GB sampled peak process-tree RSS.
Cold publication took150.985s with7.7044GB peak RSS, then fresh inspection
confirmed9,982 published rule records and current scope. The resident index
remained active during solve and cold publication. Evidence:
`TMP/r0-public-smoke-4iB73F/verification.json`. These are shared-host correctness
measurements, not a speedup claim or proof of completion at higher ranks.

Repaired optimized binary:
`TMP/releases/20261016-finite-resident-fixed/rustred`, SHA256
`d044f0a6e332fa57306de13634c27ef9fc956a762f9bae122f0943ba1f9070d0`.
Full build, focused tests and terminal evidence are recorded in `CODEX_PROGRESS.md`.

### Historical configurable-limit release

The results below describe the preceding configurable-limit release. Its K1
storage stress had the resident cache disabled and did not exercise the
uncapped-directory hydration failure. They are not evidence that the new
resident-accounting repair has passed the full five-loop high-memory profile.

The native storage, lifecycle, master publication/refinement, saved application,
CLI and dashboard gate passed319 tests (four explicitly ignored fixtures).
Three additional public codec tests passed: above64MiB payloads, nested
allocation accounting compared with bincode, and hostile length/integrity
rejection. The app test build used an unoptimized app over optimized core;
these are correctness tests, not timing claims. The delivered CLI is a normal
optimized release build.

An independent release K1 storage stress completed4200 actual one-seed windows:
8400 Work records,4200 Work segments/pages, generation4201. Runtime was138.461s
with14.7MiB sampled peak RSS. Publication completed in0.164s and a fresh-process
artifact inspection in0.008s. Setting an explicit4096 run cap rejected source
inspection, publication and cold inspection without changing `CURRENT`,
`latest.json` or `artifact.json`. This is an IO regression, **not** a new
five-loop solve or a representative throughput benchmark.

Evidence remains locally under:

- `TMP/finite-user-limits-tests3-20261016`
- `TMP/finite-user-limits-codec-tests-20261016`
- `TMP/finite-user-limits-k1-stress-20261016.uRAO5M/evidence.json`

The independent implementation reviews covered policy forwarding, source
identity, lower-limit refusal, cold loading, checkpoint preservation and UI
counter semantics. The real-PTY visual check also caught the distinction between
Ratatui's full-terminal `CompletedFrame.area` and its actual inline buffer area;
cleanup now uses the latter. Frozen-release terminal evidence and executable
identity are recorded in the final `CODEX_PROGRESS.md` entry.

Earlier frozen CLI, before the resident-accounting repair:
`TMP/releases/20261016-finite-user-limits-final/rustred`, SHA256
`fbad42ea34a211653aae11fb059676a819d64619980a583b63dbeb716b8f4819`.
The final viewport-only cleanup rebuild passed three actual-native PTY checks
and fresh cold inspection/refusal against the4200-run stress artifact.

## Fresh five-loop launch

At the user's request, a separate campaign was initially staged as **inputs only** at
`campaigns/five-loop-finite-r0-d10-configurable-20261016`. It retains all67 saved
owners,116 required queries and the authenticated original183-query attachment
for later extensions. R0 query bytes match the previous campaign. Its subsequent
attempt stopped during initial resident accounting; the namespace now exists
and must not be restaged over or removed. The earlier campaign remains separate.

The user prefers a fresh start. A separate inputs-only namespace is prepared at
`campaigns/five-loop-finite-r0-d10-resident-fixed-20261016`; the failed namespace
is retained untouched. After validation of the repaired executable, use:

```bash
cd /shared/codex-2/rustred
export RUSTRED_CAMPAIGN=/shared/codex-2/rustred/campaigns/five-loop-finite-r0-d10-resident-fixed-20261016
export RUSTRED_EXECUTABLE=/shared/codex-2/rustred/TMP/releases/20261016-finite-resident-fixed/rustred

nix develop --command python -B examples/python/saved_campaign.py run \
  --campaign "$RUSTRED_CAMPAIGN" --engine finite-demand \
  --executable "$RUSTRED_EXECUTABLE" \
  --finite-policy examples/input/finite_campaign/high_memory.json \
  --cpus 0-31 --max-memory-bytes 600000000000 \
  --memory-policy process-limit
```

The Symbolica license must already be in the environment. Use the tracked Git
flake, not `nix develop path:...`, which can copy ignored campaign data. The RAM
ceiling is requested, not reserved, and remains 600 GB in `process-limit` mode.
The dashboard reports host available RAM separately. To request the earlier
host-based admission behavior, explicitly select `--memory-policy host-aware`
and a `--host-memory-reserve-bytes` floor. Swap-growth stopping remains disabled.

Once each stage finishes, increment the requested rank without regenerating
rules or discarding the completed Work index:

```bash
nix develop --command python -B examples/python/saved_campaign.py extend \
  --campaign "$RUSTRED_CAMPAIGN" --rank 1
nix develop --command python -B examples/python/saved_campaign.py inspect \
  --campaign "$RUSTRED_CAMPAIGN"
```

Use ranks2,3,… subsequently; omitting the D option retains D≤10. Original
topology-specific bounds still apply: these maxima are not a blanket statement
about every integral below them. After interruption, use `run --resume`.
Refinement remains an explicit user action:

```bash
nix develop --command python -B examples/python/saved_campaign.py refine \
  --campaign "$RUSTRED_CAMPAIGN" --seed-depth 1 --containing-sector-depth 1
```

To change an operational grant later, supply a complete policy file with
`run --resume --finite-policy FILE`; do not edit a frozen file under `bin/`.
Inspection can use `--finite-policy FILE` as a read-only override. The selected
run policy is automatically forwarded to later publication and refinement.
