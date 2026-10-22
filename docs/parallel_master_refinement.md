# Parallel exact master refinement

The finite terminal-relation search has a native work-stealing executor.
This changes execution, not source selection, input coverage or master policy.
Ordinary serial `TerminalRelationSession` APIs remain available. Refinement
uses the parallel executor when its requested thread budget exceeds one;
publication-only execution is unchanged.

## Current delivery: 22 October

The assisted executor now genuinely parallelizes ordinary exact IBPs even when
finite numerator-symmetry assistance is enabled. Its two work-stealing stages
prepare sources and reduce exact blocks against immutable U; the final exact
join remains ordered. A configured worker count is not a claim of CPU use.

The final `capacity-dispatch` release is
`TMP/releases/20261022-parallel-refinement/rustred`, SHA256
`70dd9ee7d41ee32a10b0b23470b73a5552f0be5f7e645cad8d08fd57105c21a1`.
The matching source/driver is on `codex/numerator-symmetry-refinement` in
`TMP/numerator-symmetry-refinement-20261021`. Use its adapter, not the older main
adapter. No production campaign was started, stopped or modified by validation.

Validation: **677 native passes, zero failures, five explicit manual ignores**;
**218 Python passes and one existing affinity skip**; 12 fresh-search and six
recovery CLI controls, real SIGINT/checkpoint/changed-worker continuation, and
reverse continuation through the older executable. Independent mathematical,
implementation and lifecycle audits passed. Native functional frontends use
unoptimized test code with the matching optimized dependencies; their runtimes
are not solver benchmarks. The optimized application build took 6m18s, excluded
from all timings.

The newer bulk join registers each committing block's new-column prefix once,
then passes its owned CSR to Symbolica. It never admits columns from a future
block. All dimensions, source support and guards are preflighted; a conservative
allowance miss falls back to per-row admission, rather than rejecting valid work.
Cancellation/checkpoint boundaries occur before/after a mutating block. There
is no fallible return within that mutation; panic/OOM is not transactional
rollback and requires restoring the last durable checkpoint. Reuse-only blocks
retain the old per-row cancellation points.

The command examples and measurements below dated 21 October are historical;
they do not describe the final assisted path. The current sourceable release
helper and final hot-window results are documented in the 22 October section
at the end of this file.

## Algorithm and exact authority

### Assisted execution and finite providers (22 October update)

Earlier assisted refinement bypassed the parallel executor: the presence of an
equation provider delegated every step, including ordinary IBPs, to the serial
session. A configured 128-worker pool therefore did not imply that the assisted
run actually used it. This integration fixes that dispatch, rather than claiming
that changing a worker option alone fixed the old executable.

`TerminalRelationFiniteProvider` owns a complete immutable map of authorized
equations. The app constructs it from verified numerator/circuit plans only when
all composed providers are finite; a saved-rule callback remains dynamic.
Off-map keys are known empty under this explicit provider authority, not from
sampling, an incomplete cache or a numerator-rank guess. Old pending off-map
queries can be removed; accepted rows, queued equations, historical queried keys,
source cursors, exact maps and conditions are retained. The runtime map is
revalidated/rebound after a cold load without changing the native checkpoint
schema or existing mathematical provider binding.

Finite assisted ordinary IBPs use the work-stealing stages below. Mutable dynamic
callbacks are never called concurrently: provider requests and equation insertion
form ordered barriers. A cheap next-source preflight avoids preparing an entire
speculative window if that source immediately opens a new provider request.
Larger ordinary windows are truncated before crossing such a barrier.

The column plan borrows the existing end-relative registry and owns only new
window-local columns. It does not clone or rebuild the complete historical
million-column dictionary for every small batch. Exact arithmetic still belongs
to Symbolica; these changes are scheduling, provenance and index bookkeeping.

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

The dashboard distinguishes configured worker budget from process-measured CPU
use. Actual native row preparation/elimination are parallel phases; mutable
provider calls are barriers, mapping and proved-row insertion are serial, and
ordered merge is labelled separately. Missing native stage is reported as
unknown, never guessed as `preparing`. Collection/completion hides the preceding
batch's now-stale counters. `provider_keys_filtered` counts pending requests
excluded by the finite map in that native call, not all avoided lookups.

The overall relation-work counter includes ordinary sources, rebuilds, provider
requests and provider rows. It is not a count of equally expensive independent
jobs, and dynamic providers can add requests. The fixed ordinary-source cursor
is the matched-work boundary used in performance measurements.

Local block reduction may produce a different exact forward-echelon U, since
Symbolica does not fully back-reduce every inserted row. This is not a changed
row space: every residual differs from its source only by frozen U and earlier
rows in that block, whose spans are already in the ordered global prefix when
it commits. Preserve every auxiliary residual, guard and original support.
Terminal-map/guard equality, source-prefix equality and exact span invariants
are the correctness claims; arbitrary assisted native byte identity is not.

## Interfaces and resume

The core exports `TerminalRelationExecutionOptions`, `TerminalRelationExecutor`
and `TerminalRelationExecutionProgress`; the executor provides `step`,
`step_observed`, observed dynamic-provider execution and
`step_with_finite_provider_observed`. Application options expose `threads`, `relation_batch_rows`
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

## 22 October: assisted hot-window results and workflow

The release-profile helper cold-loads an independent immutable copy of the real
five-loop generation-8 assisted session and commits the same next **256 ordinary
sources**, cursor 639637 -> 639893, using batch 256/block 8. The frozen session
already contains approximately 607k independent rows and 1.45M columns. These
are fixed-prefix controls, not completed whole refinements. Runs are serialized
on physical CPUs 224-255 on a shared host; compiler/setup time is excluded from
the solver timer. Whole-process wall/CPU/RSS include loading, proof replay,
encoding and cold verification. Verification includes encoding; do not add the
two timers twice. GB below is decimal.

| Final build mode | Workers | Solver, s | Whole process, s | Process CPU, s | Peak RSS, GB |
| --- | ---: | ---: | ---: | ---: | ---: |
| Original serial provider | 1 | 108.610 | 153.122 | 151.098 | 11.942 |
| Finite-provider serial | 1 | 108.975 | 153.125 | 151.396 | 11.940 |
| Finite parallel | 6 | 76.946 | 122.666 | 174.676 | 14.769 |
| Finite parallel | 16 | 52.815 | 99.824 | 216.430 | 19.383 |
| Finite parallel, unprofiled | 32 | 57.061 | 103.628 | 352.632 | 26.191 |
| Original serial provider, jemalloc | 1 | 105.464 | 149.734 | 148.170 | 10.751 |
| Finite parallel, jemalloc | 16 | 47.092 | 94.212 | 177.262 | 15.854 |
| Original serial provider, jemalloc, repeat | 1 | 105.815 | 151.520 | 149.893 | 10.750 |
| Finite parallel, jemalloc, repeat | 16 | 47.301 | 92.411 | 174.576 | 15.649 |

Every arm retains the same 939 exact terminal maps, 108,524 guards, 864 basis
keys and committed source prefix. Native cold loading passes. Parallel runs
have identical native states; serial and parallel use different but equivalent
upper-echelon representations. Final parallel U has 607,556 rows, 1,454,289
columns and 13,024,941 nonzeros versus serial 13,024,869. All-map/guard comparison
and the auxiliary-row span argument—not equal counts or sampled zeros—support
correctness. These particular hot maps predate the user's later 862-key import.

The initial final-build 16-worker result is 2.06x solver / 1.53x whole-process
faster than serial. The two matched same-allocator pairs reproduce **2.239x and
2.237x solver speedups**, or **1.589x and 1.640x whole-process speedups**.
The selected setting is 16 workers, batch256/block8, with the native-only pinned
jemalloc wrapper. A 32-worker allocator trial took47.572s, essentially tied with
16, but used21.803GB versus15.854GB and more CPU. This is not evidence of linear
scaling to128 workers or of a full-refinement speedup. In the earlier
64-source cohort, two six-worker pairs reproduced 2.67x solver gains, but those
prototypes lacked the final production-compatible capacity-dispatch feature and
are not silently pooled into the final table.

Negative results matter. Reducing blocks from eight to two exposed 32 jobs and
about 31 active cores, yet increased wall/CPU/RAM. Splitting 256 sources into
four 64-source windows improved six-worker time (81.995 -> 64.087s) but worsened
32-worker time (61.585 -> 67.001s) on the earlier graph. More frequent fresh-U
joins therefore are not universally better. R2's observed 6/16-worker times are
about 6-7% lower than the first implementation, not an order of magnitude. That
comparison also changes the capacity-dispatch feature profile, so it does not
isolate a causal bulk-join gain. Serial/parallel comparisons within each graph
are matched.

Final K6/H/FG/BMW/X controls each commit the same128-source prefix through
serial and six-worker paths, checking exact maps, nonempty guards, cursors and
unchanged inputs. Raw keys/basis/guards respectively are38/5/17,386/21/420,
145/15/731,179/16/825 and445/18/1449. These4-13ms solver cohorts are correctness
controls, not lower-loop throughput claims. The final independent hot comparison
also directly verifies all939maps and108,524guards under the selected allocator;
the coarse-block negative probe passes the same exact comparison.

Own-process profiles identify the remaining limits: local frozen-U coefficient
copying/allocation (about 49% malloc-related self samples and 16% integer-vector
clone in the sampled parallel stage), and exact Symbolica polynomial division/
GCD at the serial join. `add_cols` itself was only about 2% of join self samples.
These ten-second samples do not prove whole-run fractions or Arc contention.
No competing CAS, host-wide allocator change or Symbolica patch is included.

The first local jemalloc candidate exported `_rjem_` symbols: loader inspection
proved it did not replace Rust's malloc, so no benchmark conclusion was drawn.
The accepted existing unprefixed library is
`/nix/store/gg44883i8mz5kkn5ziccv9h62zaqn8mm-jemalloc-5.3.1/lib/libjemalloc.so.2`,
SHA256 `1d12cb7be363c4b69c4f065e7f583a5e9d231410a572fe7aaabcf193602e7418`.
Actual malloc/free loader bindings and native process mappings were checked.
The frozen wrapper verifies native/library hashes and rejects an unexpected
preload; it execs the native binary with the same supervised PID. A real
dispatcher/RAM-guard pause/resume check confirmed native alone maps the library,
not Python; changed-worker continuation and exact cold maps passed. It does not
install a system allocator. The direct new binary remains a no-preload fallback.

Detailed local evidence is under
`TMP/parallel-assisted-refinement-20261022/`, including
`R2_HOT_WINDOW_RESULTS.md`, `final-cap/README.md`,
`CLI_ACCEPTANCE_R2_RESULT.md` and `REVERSE_COMPATIBILITY_RESULT.md`.
Generated data and executables are deliberately not committed.

### Current campaign commands

The user's proof import has already completed: scope `528866...` preserves all
maps/guards and changes the current basis **864 -> 862**, without ordinary IBP
generation. Do not repeat recovery or import. Start a new exact `(2,0)` round
from that publication using the new release helper:

```bash
cd /shared/codex-2/rustred
source TMP/releases/20261022-parallel-refinement/workflow.sh
rrfresh 2 0
```

The helper uses the matching feature adapter, exact strategy, finite numerator
assistance, no dynamic saved-rule assistance, a 600 GB process RAM grant,
16 workers/CPUs 64-79, batch 256/block 8, and no explicit column ceiling.
These worker/window choices are mutable operational settings, not mathematical
search or inventory caps. They can be overridden with the corresponding
`--master-refinement-*` / `--master-relation-*` arguments. The helper selects
the checked native-only allocator wrapper; use
`export RUSTRED_BIN="$RUSTRED_NATIVE_BIN"` for the direct-binary fallback.
The new round retains proved relations but intentionally does
not retain the old search matrix; earlier checkpoints remain on disk.

After Ctrl+C, wait for the checkpoint acknowledgement and process exit. Resume
the **same** round with `rr --refine-masters` (omit fresh). Inspect at any time
with `rrinspect`. To retain interrupted maps and publish without advancing
source work, use `rr --finalize-refinement`. Changing to a deliberately fresh
shape, for example `(3,0)`, requires finalizing first, then `rrfresh 3 0`.
Never run two writers for the same campaign.
