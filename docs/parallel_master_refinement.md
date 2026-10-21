# Parallel exact master refinement

The finite terminal-relation search has a native work-stealing executor.
This changes execution, not source selection, input coverage or master policy.
Ordinary serial `TerminalRelationSession` APIs remain available. Refinement
uses the parallel executor when its requested thread budget exceeds one;
publication-only execution is unchanged.

## Algorithm and exact authority

1. Specialize and normalize an operational source window in the existing
   licensed SectorExecutor/Rayon pool, sharing immutable family/source data.
2. Simulate the serial first-encounter column order without changing the session.
3. Dispatch stealable elimination blocks. Each copies only reachable frozen-U
   pivot rows into compact column storage. Symbolica's public CSR constructor,
   `SparseRowReducer::from_upper_triangular_matrix` and `add_row` perform all
   elimination; RustRed adds graph reachability and scheduling, not a CAS kernel.
4. Return every independent residual, including auxiliary-integral equations.
   Each is a combination of its source, frozen exact U and earlier block rows.
5. Merge in source order, admitting ORIGINAL support even if a local combination
   cancels it. Preserve source conditions. Advance cursors only on committed rows.

For example, `x + M1 = 0` and `x + M2 = 0` imply `M1 - M2 = 0` only when
combined. Discarding auxiliary equations would lose that relation. Local
dependence is established by exact Symbolica arithmetic, not sampled zeros.

Ordinary sources and inventory-extension rebuilds use this path. Mutable
external equation providers retain their serial authority/request order. Final
collection, aliases and finite-feedback publication retain their existing paths.
No claim is made of full CPU saturation there or inside a serial CAS operation.

Checkpoint schemas are unchanged. In-flight blocks are disposable; cancellation
drains started operations and leaves a valid committed prefix. Uncommitted work
is recomputed on resume. Workers and batch/block sizes are operational choices,
not mathematical search identity. The existing phase dispatcher supports an
explicitly replaced refiner and authenticates its unchanged source/search recipe.

An observer on the caller's original thread reports atomic preparation,
elimination and merge counters while computation runs. These scratch counters
are not durable source counts or closure claims. CLI/Python telemetry renders
them in a separate coloured dashboard row.

## Interfaces and resume

The core exports `TerminalRelationExecutionOptions`, `TerminalRelationExecutor`
and `TerminalRelationExecutionProgress`; the executor provides `step` and
`step_observed`. Application options expose `threads`, `relation_batch_rows`
and `relation_block_rows`. Native `walk-master-reduce` accepts `--threads`,
`--relation-batch-rows` and `--relation-block-rows`. The public wrapper transports
the last two through both legacy and finite campaign refinement. Omitted sizes
retain saved preferences or native defaults. Positive sizes are scheduling
windows, not total-work ceilings. The existing aggregate RAM guard still applies.

After gracefully interrupting refinement and waiting for its checkpoint and exit:

```bash
cd /shared/codex-2/rustred
export RUSTRED_CAMPAIGN="$PWD/campaigns/five-loop-minimal-r0-d10-incremental"
nix develop --command python -B examples/python/saved_campaign.py refine \
  --campaign "$RUSTRED_CAMPAIGN" \
  --workers 128 --cpus 64-191 \
  --seed-depth 1 --containing-sector-depth 1 \
  --relation-batch-rows 4096 --relation-block-rows 8 \
  --executable "$PWD/TMP/releases/20261021-parallel-master-refinement/rustred"
```

Refiner-only worker/CPU overrides do not rewrite the closure engine's frozen
policy. CPU IDs 64–191 are 128 distinct physical cores on this host and do not
overlap the six-loop run's 192–223 allocation. A one-row admission control passed
with the actual licensed 128-worker pool; this is not a 128-core speed benchmark.
The command inherits RAM grants, retains the unfinished exact cursor, and leaves
the last completed publication available. Do not run two dispatchers for the
same campaign. No production run is automatically stopped or restarted.

## Matched release prefix measurements

The immutable input is generation-119 R6/D10 publication `bf83f6b4…`, native
state generation three: 939 raw keys, 51,250 independent rows, 30 terminal
relations. The generic `terminal_refinement_probe` example executes the same next
64 source rows. Loading, encoding and diagnostics are outside the solver timer.

| Workers | Block rows | Solver seconds | Process CPU seconds | Peak RSS, GiB |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 8 | 145.576 | 148.04 | 0.750 |
| 32 | 8 | 63.354 | 155.62 | 2.278 |
| 32 | 2 | 57.508 | 225.30 | 6.410 |
| 1, repeat | 8 | 145.593 | — | — |
| 32, repeat | 8 | 62.684 | 153.37 | 2.276 |

All produce native state BLAKE3
`2f5db653f8a209a733c1a76f34cfa7ffbe1c3d6f6688962bfbdfb87adfd79b30`,
the same terminal-map digest, 51,280 independent rows, 1,188,814 sparse nonzeros
and 909 remaining candidate masters. Eight-row blocks give 2.30× observed
solver speedup at modest additional CPU cost; two-row blocks give 2.53× but cost
more CPU/memory. There are only eight blocks in the first parallel control and
32 in the second. Larger production windows expose more jobs, but neither a
whole-refinement speedup nor sustained 32-core utilization is inferred.
All use affinity 224–255 on a shared, non-isolated host; compilation is excluded.
The repeated eight-row pair gives 2.32× speedup. 54 focused native relation
tests pass, including exact serial/parallel checkpoint identity, auxiliary
cross-block relations, scope-extension rebuilds and cancellation. 165 Python
controls pass. An actual native CLI run pauses after a committed prefix with
four workers, then resumes with eight, advancing generations 3→5 and work
319318→319334. The source publication remains unchanged. These are correctness
and prefix timing controls, not a completed full refinement.
Local evidence: `TMP/master-refinement-parallel-20261021/`, not committed output.

Optimized delivery: `TMP/releases/20261021-parallel-master-refinement/rustred`,
SHA256 `8afc9d5ceba1124cb9077de09085f041d9e5f28be82b09163bc212e7f816580a`.
The default `target/release/rustred` is refreshed too; running frozen binaries
are not overwritten. Compilation is separate (7m38s for the final CLI build).

## Understanding the inherited progress

The R6 run inherited 54,909 ordinary equations, 51,250 independent rows and
264,393 rebuild operations. Containing-sector depth 0→1 changes 24,010 seeds
to 24,151: 141 × 25 = 3,525 extra sources, only 0.4% of the 868,168-work total.
Most pending work was already unfinished seed-depth-one search. Exact row
cost grows with the row space; a work fraction is not a reliable ETA.

## Modular/reconstruction prospects

They are plausible future backends, especially for coefficients mainly in `d`.
Modular probes can discover rank/pivot/source supports; Symbolica's public
rational-function reconstruction can propose exact coefficients. Sampled rank
or zeros alone cannot prove relations. Such a backend must provide exact
identity/provenance validation and preserve guards. This implementation remains
exact Symbolica arithmetic throughout: no CRT/interpolation/reconstruction kernel
is added to RustRed.
