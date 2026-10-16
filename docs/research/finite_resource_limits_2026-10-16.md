# User-controlled finite campaign limits and dashboard

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

The finite solve now has a bordered, colored Ratatui table with preparation
counts, committed required-row progress, checkpoint/window details, measured
CPU cores versus reserved workers, memory and rates. Window changes reset
window-local rate sampling. Startup is a separate colored phase/elapsed line;
publication retains its existing table. Error output follows the table rather
than overwriting it. Non-TTY operation uses bounded periodic progress lines.

## Validation and limits of the evidence

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

Final frozen CLI:
`TMP/releases/20261016-finite-user-limits-final/rustred`, SHA256
`fbad42ea34a211653aae11fb059676a819d64619980a583b63dbeb716b8f4819`.
The final viewport-only cleanup rebuild passed three actual-native PTY checks
and fresh cold inspection/refusal against the4200-run stress artifact.

## Fresh five-loop launch

At the user's request, a separate **inputs-only** campaign is prepared at
`campaigns/five-loop-finite-r0-d10-configurable-20261016`. It retains all67 saved
owners,116 required queries and the authenticated original183-query attachment
for later extensions. R0 query bytes match the previous campaign. No native
production process has been launched and the old campaign remains untouched.

After the release is delivered:

```bash
cd /shared/codex-2/rustred
export RUSTRED_CAMPAIGN=/shared/codex-2/rustred/campaigns/five-loop-finite-r0-d10-configurable-20261016

nix develop --command python -B examples/python/saved_campaign.py run \
  --campaign "$RUSTRED_CAMPAIGN" --engine finite-demand \
  --executable /shared/codex-2/rustred/TMP/releases/20261016-finite-user-limits-final/rustred \
  --finite-policy examples/input/finite_campaign/high_memory.json \
  --cpus 0-31 --max-memory-bytes 600000000000 \
  --host-memory-reserve-bytes 150000000000
```

The Symbolica license must already be in the environment. Use the tracked Git
flake, not `nix develop path:...`, which can copy ignored campaign data. The RAM
ceiling is requested, not reserved; the host reserve can reduce the effective
allowance. Swap-growth stopping remains disabled.

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
