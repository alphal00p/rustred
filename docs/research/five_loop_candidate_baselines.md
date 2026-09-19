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
template is `TMP/five_loop_next_run_template.sh`; the setup and final census
received an independent audit. No Möbius or reconstruction comparison was
launched as part of this baseline, and no speed comparison with either is
claimed.
