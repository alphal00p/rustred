# Five-loop candidate-generation baselines

## Physical cube: bounded sparse-exact run, 2026-09-19

**The complete requested family did not finish within 30 minutes on six
workers.** The run ended at its predeclared wall-time limit, not at a successful
save or a certification gate. No candidate bundle was written. This is a
time-censored baseline, not a completed generation timing or a closure claim.

The input is external data in [`five_loop_cube.toml`](../../examples/input/five_loop_cube.toml):
five loop momenta, twelve physical unit-mass propagators and three auxiliary
coordinates completing the K=15 scalar-product basis. The explicit
`--nonpositive-indices 12,13,14` restriction keeps those three numerator
coordinates nonpositive. The physical root is `111111111111000`; the request
includes its nonzero subsectors, not just one integral or the all-positive
15-coordinate sector. No topology-specific solver code was added.

### Frozen workload and bounds

| Item | Setting |
|---|---|
| CLI | Existing optimized `target/release/rustred`, version 0.1.0 |
| Backend | `family-candidates --exact-backend sparse` |
| Ordering | Natural default; no permutation supplied |
| Workers | Six, CPU affinity 88–93 (six distinct physical cores) |
| Nested pools | Rayon, OpenMP and BLAS pools capped at one |
| Deadline | 1,800 s; TERM, then KILL after 10 s if necessary |
| Memory limit | 34,359,738,368 bytes of **virtual address space**, not RSS |
| Certification | None requested or executed |
| Host | Shared; cores were disjoint from other concurrent experiments |

Executable SHA-256:
`7adbf2cbe80c092d9b41b8a17d7a421e5757fd966c8ffc2e7cdf43a66968d377`.
Input SHA-256:
`0247e0d8fab665a59d34cbfac0d20f94ea33e282e926870842edaaf53fcac77f`.
Both were verified before and after execution. The executable does not embed a
Git revision; its provenance is recorded by its actual hash, not an inferred
source commit. There was no compilation, rule reuse, FORM oracle, or runtime
change to the input, ordering, backend or deadline.

### Measured outcome

| Measurement | Result |
|---|---:|
| Termination | GNU `timeout`, exit status **124** |
| Whole command wall time | **1,800.75 s** |
| User CPU | 10,626.24 s |
| System CPU | 20.56 s |
| Total CPU | 177.45 CPU-minutes; reported utilization 591% |
| Peak RSS | **7,946,512 KiB (7.58 GiB)** |
| Swaps | 0 |
| Saved candidate bundle / timing report | **Neither produced** |

These resource measurements include preparation and shutdown. No solver-only
completion time can be extracted from a report that was never written.
Preparation's progress event reported **16.1 s**, with **2,656 nonzero physical
sectors**, **1,440 scoped-zero sectors**, and **5,480 global zero proofs**. The
first two counts sum to the root's 4,096 masks; the global zero census also
examines masks outside that root and is therefore larger.

Before termination, the progress log contains **37 distinct completed-sector
events**, reporting a total of **9,818 rules** and **92 finite-residual
occurrences** in those sectors. There are no duplicate completion events.
These are partial in-memory solver results reported as events, **not retained
candidate payloads**. The residual count is not a deduplicated master count.
The full physical parent (sector mask 4095) did not complete. The final sector
completion event occurred at 1,740.9 s; search/lifting events continued through
1,800.0 s.

Six named sector workers remained CPU-active. Manual RSS observations rose from
approximately 0.68 GiB at one minute to 5.08 GiB at fifteen minutes and 7.54 GiB
near the deadline. The process never approached the 32 GiB virtual-memory cap
in these observations. Progress included depth-three source searches and
exceptional-case queues with at least 136 pending entries. Ordinary generation
events are throttled, so these maxima are observed values, not exhaustive
censuses. Their frequency does **not** measure where CPU time was spent.
This run did not collect a CPU profile.

The timeout and solver PIDs (2647401 and 2647402) were independently checked
absent after termination. No owned workers remain. There is no saved output to
cold-load, inspect for exact consistency, apply, or claim as a completed family.
The measurements establish the cost of this bounded attempt; they do not show
that eventual completion is impossible, nor predict its eventual runtime.

### Reproduction

Use six available physical cores; 88–93 are the allocation used here. Supply the
current Symbolica license only in the environment. All evidence, cache and
temporary paths remain in the workspace. Freeze the binary before starting and
retain its hash; a different build is a different measurement.

```bash
cd /common/dev/rustred
: "${SYMBOLICA_LICENSE:?supply the current license}"
export CARGO_HOME="$PWD/TMP/cargo-home" TMPDIR="$PWD/TMP"
export SYMBOLICA_HIDE_BANNER=1 RAYON_NUM_THREADS=1
export OMP_NUM_THREADS=1 OMP_THREAD_LIMIT=1 OMP_DYNAMIC=FALSE OMP_MAX_ACTIVE_LEVELS=1
export OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 BLIS_NUM_THREADS=1
RR_RUN=$(mktemp -d "$PWD/TMP/five-loop-cube.XXXXXX")
cp --reflink=auto target/release/rustred "$RR_RUN/rustred"
cp examples/input/five_loop_cube.toml "$RR_RUN/input.toml"
sha256sum "$RR_RUN/rustred" "$RR_RUN/input.toml" > "$RR_RUN/inputs.sha256"
if /usr/bin/env time -v -o "$RR_RUN/time.txt" \
  timeout --signal=TERM --kill-after=10s 1800s \
  prlimit --as=34359738368 -- taskset -c 88-93 "$RR_RUN/rustred" family-candidates \
  --input "$RR_RUN/input.toml" --input-format toml \
  --nonpositive-indices 12,13,14 --n-cores 6 --exact-backend sparse --progress \
  --output "$RR_RUN/candidates.bundle" --report-output "$RR_RUN/report.toml" \
  > "$RR_RUN/stdout.log" 2> "$RR_RUN/progress.log"; then
  RR_STATUS=0
else
  RR_STATUS=$?
fi
printf '%s\n' "$RR_STATUS" > "$RR_RUN/status.txt"
```

Use GNU `time`, not a shell builtin. The local execution used its resolved Nix
store path, preserved in the actual command/script. This CLI's default output
admission also limits a candidate bundle to 256 MiB and one million structural
collection entries. Those limits were not reached here because solving never
finished; a later encoding failure must not be mislabeled as successful
generation. A genuinely successful save would still require a separate native
cold-load and exact application check before being called a usable result.

Local raw evidence is retained under
`TMP/five-loop-candidate-next.tRR9BV/`: frozen CLI/input, hashes, progress,
stdout, exact status, GNU resource report, and manual observations. The executed
template is preserved as `run-template.sparse.sh` inside that evidence folder;
the setup and final census received an independent audit. No Möbius run was
launched. The separately authorized reconstruction comparator is recorded below.

## Matched semi-numerical comparator

After the sparse run and its independent audit, one sequential comparison used
the **same frozen CLI hash, input bytes, physical root, default ordering, six
workers, CPU affinity and resource limits**. Only `--exact-backend` changed to
`semi-numerical`, and outputs used a fresh directory. This exercises the existing
Symbolica-backed reconstruction route; no solver or algebra code was changed or
rebuilt. It is unrelated to the separate factorized-coefficient experiment in
the rule **application** cache.

**This run also reached the 1,800-second deadline without a saved bundle.** Its
timeout and solver PIDs (2918431 and 2918432) were checked absent afterward.
The actual backend selection, hashes, limits, affinity, cleanup and following
resource/count table were independently audited.

| Whole bounded attempt | Sparse exact | Semi-numerical |
|---|---:|---:|
| Terminal status | Timeout, 124 | Timeout, 124 |
| Measured wall | 1,800.75 s | 1,800.64 s |
| User CPU | 10,626.24 s | 10,619.92 s |
| System CPU | 20.56 s | 18.83 s |
| Total CPU | 177.45 min | 177.31 min |
| Peak RSS | 7,946,512 KiB (7.58 GiB) | 6,394,316 KiB (6.10 GiB) |
| Preparation event | 16.1 s | 16.4 s |
| Completed-sector events | 37 | 41 |
| Rules in those events | 9,818 | 8,591 |
| Finite-residual occurrences | 92 | 63 |
| Full physical parent completed | No | No |
| Saved bundle / phase report | Neither | Neither |
| Cold-load or application check | Not possible: no output | Not possible: no output |

The semi-numerical preparation produced the identical sector/zero census. Its
last completion event occurred at 1,408.8 s; search/lifting events continued
through 1,799.8 s. No swaps were reported and stdout is empty. Its progress
stream contains depth-three searches and pending-case counts up to 107; as
above, throttled intermediate events do not give exhaustive maxima or a profile.

The two completed-sector sets **differ**. They overlap in 24 sectors, all of
which have matching rule and residual counts. This is a limited count
consistency observation, not an exact comparison of saved equations: neither
run retained a candidate payload. More completed sectors but fewer rules do not
rank the backends, and the lower measured peak RSS does not establish a memory
ratio for an identical completed workload. Neither backend has a completed
full-family generation timing in this experiment; no speedup is inferred from
progress percentages, partial output counts, or these censored wall times.

### Important solver-internal validation boundary

The current semi-numerical materializer reconstructs the target row using
Symbolica and then compares it with a characteristic-zero sparse target-row
materialization of the selected source trace. If that exact row was already
computed for support recovery it is reused. This check remains **inside the
generation backend**, before returning the row. It is separate from the
optional independent artifact-certification pass, which was not requested here.
Consequently `semi-numerical` is **not reconstruction-only timing** and does not
remove all sparse exact-lifting work from this comparison.

The materializer is implemented in
[`solver/discovery/semi_numerical.rs`](../../crates/rustred-core/src/solver/discovery/semi_numerical.rs),
whose last recorded change before the frozen CLI build was `b77c0646`.
These measurements predate the observer-only additions that now expose replay
boundaries. The frozen executable
also contains that path's distinctive exact characteristic-zero replay failure
messages. This corroborates the retained validation path without inventing a
full build-revision identifier. No validation was disabled or weakened during
the paired runs, and their coarse progress logs do not split reconstruction,
exact replay, canonicalization or other arithmetic time.

To reproduce the comparator, use the command above with **only**
`--exact-backend semi-numerical` substituted and a fresh evidence directory.
The actual script and raw evidence are retained under
`TMP/five-loop-candidate-next.718HJ6/`. Both generation processes are stopped;
no further full-family campaign was launched. These unfinished five-loop
attempts do not change the separate validated four-loop numerical application
results or settle the separate optional bounded-rank certification work.

## Selected-sector follow-up: identify the cost before changing the algorithm

A subsequent, independently audited diagnostic uses the unchanged external
cube input and ordering, but solves only two preselected sectors: mask **3734**,
the first completed sector in both original logs, and the unfinished physical
parent **4095**. It links existing optimized libraries containing the new replay
observer; it does not reuse the frozen old CLI or repeat all 2,656 sectors.
Each backend/sector runs on one CPU (82), with a 180-second deadline and the
same 32 GiB virtual-memory bound. The parent additionally uses 99-Hz userspace
sampling. Observer I/O and profiling make these **diagnostic measurements**,
not new production or completed-family timing claims.

| Selected scope | Sparse exact | Semi-numerical |
|---|---:|---:|
| Mask 3734 result | 177 rules, 1 finite residual | Same |
| Mask 3734 solver core | 6.668 s | 8.641 s |
| Mask 3734 exact replay inside materialization | — | 0.224 s |
| Mask 4095 terminal status | Timeout, 124 | Timeout, 124 |
| Mask 4095 whole command wall | 180.56 s | 180.37 s |
| Mask 4095 completed symbolic-rule events | 281 | 274 |
| Mask 4095 last observed solver event | 161.439 s | 159.142 s |
| Discovery through that event | 29.841 s | 23.514 s |
| Materialization excluding semi-numerical replay | 130.476 s | 51.347 s |
| Completed internal exact replay intervals | — | 83.602 s |

The completed representative passes a separate **6,898-coefficient exact
comparison**, including ordered contexts and the full case/source/guard/RHS/
residual structure. Its symbolic discovery dominates; reconstruction does not
provide a speed improvement in this one diagnostic observation. Both parent
processes remain incomplete and have no saved complete sector output. Their
different partial counts cannot be used to infer relative completed-work
throughput or eventual closure time.

For the parent prefix, exact materialization is **80.8%** of sparse's observed
solver interval. In the semi-numerical interval, reconstruction outside replay
is **32.3%** and retained exact replay another **52.5%**. It starts 206 replay
checks, completes 205 without failure or support recovery, and is interrupted
inside the remaining replay. Sparse ends with an open materialization. Open
tails and shutdown time are not counted as completed phase intervals.

The profile agrees with the phase logs: sparse lifting and semi-numerical
replay expose native polynomial division/arithmetic and allocation, while
reconstruction exposes native finite-field GPLU scatter, row reduction,
coefficient evaluation and inversion. Guard extraction and exceptional geometry
are small in this prefix. A concrete hard coordinate case has only `n0` and
dimension active, yet selects **997 source rows and 3,458 columns**. This is
elimination cost, not evidence that optional artifact certification is blocking
generation.

The narrow next experiment is the existing `SparseTargetOnly` lifting option
on that frozen hard case, preserving exact row/context comparison and caps;
no new algebra or validation weakening is implied or implemented here. Native
reconstruction already caches complete row images across coefficients at the
same prime and point. Further optimization should target measured remaining
costs rather than duplicate that cache.

See [the detailed selected-sector report](five_loop_selected_sector_profile.md)
for complete boundaries, resources, per-symbol counts and qualifications.
Evidence and independent audit: `TMP/five-loop-sector-profile.Ijiilz/`.
All owned diagnostic clients are stopped; no further generation job was started.
