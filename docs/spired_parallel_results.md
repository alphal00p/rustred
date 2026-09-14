# Sector-parallel SpIRed reference port

Date: 2026-09-14. This measures the complete supplied `vac3` sector workload,
not a certified family artifact or completion of the PM example suite.

## Implementation

`rustred::solver::SectorExecutor` runs independent sectors on a reusable private
Rayon pool. A one-worker executor runs inline. All workers borrow the same
native-polynomial source system and share the immutable zero-sector census;
preconditioning, exceptional-case queues, GPLU state, and compact exact replay
remain sector-local. There is no per-worker clone of the entire family or
coefficient serialization between workers.

Completed solutions are consumed on their workers. The example writes each
sector's file immediately and retains only compact statistics and residual keys
unless oracle verification was explicitly requested. Aggregate outputs and the
first reported error follow input-manifest order. Active-coordinate count
descending, followed by sector lexicographic order, provides the default
structural worklist priority. Rayon controls actual start order; this is not a
strict longest-job-first dispatcher and uses no saved fixture timing hints.

All coefficient and sparse algebra remains in Symbolica/numerica. Public APIs,
their native implementations, and the actual hot paths were independently
audited. Current forward-only sparse reduction, polynomial/rational arithmetic,
GCD, and factorization introduce no nested compute pools. Native parallel back
substitution exists but is not used here. Rational-function reconstruction is
explicitly deferred until Symbolica provides it.

The thread budget applies to this executor, not unrelated application pools.
Arbitrary nested Rayon callbacks can suspend a sector frame while another runs,
so it is not a universal bound on user callback allocations. The current solver
and output callbacks do not do this. No global pool or environment settings are
changed by the library.

## Validation

- All 86 focused solver tests pass, including nine new executor tests for
  bounded threads, native license limits, inline execution, shared data,
  deterministic output/errors, and nested Rayon pool reuse.
- `cargo check --locked --workspace` passes.
- The nine executor tests also pass with the updated runtime license.
- Release K1 and K3 generation produces byte-identical rules and residuals
  across serial and requested six-worker runs; the example caps effective
  workers to the number of sectors (one and four respectively).
- Each measured `vac3` worker setting has a separate successful native exact
  comparison of all 617 equations, coordinate guard domains, and sector signs.
  Reference equations are read only after independent generation finishes.

No independence/minimality claim is made for the 38 finite numerical residuals.
The new solver's results still require connection to artifact publication and
Vakint; the full PM example suite remains outstanding.

## Measurement protocol

Both implementations use optimized release builds. Rust uses
`cargo build --release --locked -p rustred --example spired-solve-sector`.
The unchanged C++ example uses GCC 14.4.0, `-O3 -DNDEBUG -fopenmp`.
Compilation and dependency setup are excluded.

The workload is the same symbolic-mass family, nine ordinary IBP sources,
38 nonzero sectors, and numerical seed depth three. Seven fresh-process pairs
were run at each of 1, 2, 4, and 6 workers. Rust/C++ launch order alternates
between pairs. Timed runs disable reference comparison and per-case progress,
use fresh output directories, and redirect stdout/stderr. Native exact
verification runs are separate from the timing samples.

Worker budgets and matching CPU affinity are set for both implementations;
C++ additionally uses fixed OpenMP counts and disables dynamic/nested teams.
The Rust executor does not depend on the global Rayon environment count.

The host is a shared AMD EPYC 9754 system, not an isolated benchmark machine.
The first batch used CPU prefixes of `0,1,2,3,4,5`. Both implementations showed
poor two-worker scaling. Subsequent inspection found an unrelated sustained
CPU-bound job pinned to CPU 1, overlapping the entire measurement. That job
was not modified or interrupted. The original measurements remain retained.

A second, otherwise identical batch used prefixes of `2,3,4,5,6,8`, excluding
the known saturated CPU. These are distinct physical cores on the same NUMA
node, but are not reserved: CPU 2's SMT sibling also showed external activity.
Consequently neither batch establishes isolated-machine scaling or a statistical
performance guarantee. Both implementations use the same affinity within a pair.

Output and timer boundaries differ slightly. C++ writes text and binary rule
files; Rust writes text rules and diagnostics. C++'s sector timer excludes
source preparation, `family.bin`, and final `MIs.dat`; Rust's campaign timer
includes pool setup and aggregate statistics/residual output, but excludes
source preparation and oracle validation. Report process and campaign times
separately; neither is a precisely matched pure-algebra microbenchmark.

## Retained first batch: contended CPU set

| Workers | Rust median campaign | C++ median campaign | Rust median process | C++ median process |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 673.064 ms | 1440 ms | 0.70 s | 1.45 s |
| 2 | 682.715 ms | 1263 ms | 0.71 s | 1.27 s |
| 4 | 289.609 ms | 529 ms | 0.31 s | 0.54 s |
| 6 | 175.187 ms | 334 ms | 0.20 s | 0.34 s |

All 60 processes (28 pairs plus four verification runs) exited successfully.
Independent audit verified 1,216 Rust sector-file comparisons, all 32 Rust
residual lists, and all 2,184 C++ output-file comparisons against their
respective validated reference snapshots.

## Second batch: excluding the known saturated CPU

Seven-run medians, using the otherwise unchanged protocol:

| Workers | Rust campaign | C++ campaign | Rust process | C++ process | Median paired Rust/C++ campaign ratio |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 682.255 ms | 1444 ms | 0.70 s | 1.45 s | 0.470 |
| 2 | 438.995 ms | 991 ms | 0.46 s | 1.00 s | 0.446 |
| 4 | 219.644 ms | 469 ms | 0.24 s | 0.48 s | 0.470 |
| 6 | 226.914 ms | 352 ms | 0.25 s | 0.36 s | 0.580 |

The paired ratio is the median of matched run ratios, not the ratio of the two
reported medians. Rust was faster in every matched pair in both retained batches.
This is evidence for this workload, not a guarantee across all PM examples.

Rust's campaign median speedups against its current serial run are 1.55×,
3.11×, and 3.01× at two, four, and six workers. **Six workers did not improve
on four in this second batch.** Rust six-worker campaigns ranged from
177.361 to 315.098 ms; C++ ranged from 316 to 426 ms. Host/SMT contention remains
present, and the executor has not been shown to scale monotonically or optimally.

| Workers | Rust median user+system CPU | C++ median user+system CPU | Rust peak RSS range | C++ peak RSS range |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 0.67 s | 1.44 s | 9224–9284 KiB | 9236–12368 KiB |
| 2 | 0.86 s | 1.87 s | 6180–9248 KiB | 9236–12364 KiB |
| 4 | 0.81 s | 1.66 s | 6156–9264 KiB | 9240–15416 KiB |
| 6 | 0.97 s | 1.76 s | 9224–12328 KiB | 12288–15400 KiB |

Source preparation, including native initialization under the current runtime
license, has median 23–24 ms and is included in process timing. Pool setup has
median 0.279/0.389/0.478 ms at two/four/six workers and is included in campaign
timing. The low observed RSS does not establish a general high-loop memory bound.

Independent output/status audit also passed for every second-batch run. Across
both batches this checks 2,432 Rust sector files and 64 residual files, 4,368 C++
files (including binary outputs), and 120 successful process exits. All payloads
match their respective exact-validated snapshots. No timing samples were discarded.

## Profiling interpretation and next measurements

Bounded numerical-corner search remains 79–81% of summed sector phase time;
symbolic search is 14–17%. At two/four workers, phase occupancy (sum of measured
sector elapsed time divided by worker count times campaign duration) is about
99.0%/95.5%. Their CPU-time inflation is similar to C++; queue imbalance is not
the dominant explanation supported by these data.

At six workers, paired Rust CPU time is about 1.45× serial and aggregate phase
elapsed time about 1.70× serial. An optimistic repacking of observed sector
durations leaves a median 30.3 ms potential scheduling gap, but that is neither
observed idle time nor a promised improvement. We did not record worker identity,
task start/end traces, or per-thread CPU, so cache/allocator effects, frequency,
preemption, SMT interference, and scheduling cannot yet be separated.

The next parallelism experiment should add that lightweight instrumentation,
then compare the current Rayon worklist with a bounded dynamic active-first
cursor on the same workload. It must retain exact output identity and native
algebra, and must not use noisy timing samples as fixture-specific dispatch.
Within-sector GPLU remains serial; this milestone parallelizes independent
sectors rather than changing their elimination chronology.

## Local evidence and reproduction

The original batch and runner are under
`target/spired-vac3-parallel.yHeG8L/`; the second batch is under
`target/spired-vac3-parallel-quiet.3qwhZ0/`. The latter directory name is only a
convenience label, not a claim of isolated physical cores. Each run retains
stdout/stderr, GNU time records, and all generated files. These files and the
C++ reference sources are not committed.

See [the Rust examples](../examples/rust/README.md) for the executable invocation:
the final argument is the requested worker count. Reproduce comparisons using
the same binaries, sector manifests, search depths, physical-core affinity, and
fresh output directories; supply the license only through the runtime environment.
