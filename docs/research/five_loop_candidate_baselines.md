# Five-loop candidate-generation baselines

## Latest full physical cube attempt: depth zero and factorized exact fields

20 September 2026: this attempt **fails under its memory limit**, rather than
completing or reaching the wall deadline. The frozen release CLI is from published
revision `2627a53758a7307366dec3a1712340556eeaacbf`, SHA-256
`6bf597e29e141c4a50664619ae85ce41c819e8f34a7fabcee15563ebb902487b`.
The external cube input and complete physical root are unchanged from the
earlier baselines below. The new public options are `--exact-backend
sparse-factorized --numerical-depth 0`, six workers on physical cores 100–105,
natural ordering, nested pools one. Depth zero still searches each initial
fully fixed seed and exactly lifts discovered relations; it may retain more
finite residuals. No loop-count or topology-name branch selects the strategy.

The predeclared bounds are 1,800 seconds to TERM, ten seconds of KILL grace,
and 32 GiB **virtual address space**. The process aborts before that deadline:

| Whole bounded attempt | Measured outcome |
|---|---:|
| Terminal status | SIGABRT, shell **134** |
| Diagnostic | `GNU MP: Cannot allocate memory (size=8416)` |
| Wall time | **1,148.08 s** |
| User / system CPU | 5,722.72 / 51.32 s |
| Peak RSS | 33,232,084 KiB (**31.6926 GiB**) |
| Swaps reported for this command | 0 |
| Preparation event | 20.3 s |
| Completed-sector events | **96 of 2,656** |
| Rules / finite-residual occurrences in those events | 24,070 / 751 |
| Saved bundle / final report | **Neither produced** |

The last completion event is at 966.7 s and the last ordinary progress event at
984.3 s; the logs do not establish what consumed the subsequent unreported
interval. GNU time's trailing `Exit status: 0` accompanies an explicit signal-6
termination and is **not success**: the enclosing launcher returns 134. All
three owned process IDs were checked absent, both frozen hashes still match,
and no restart was performed. Independent parsing finds no duplicate completion
events. The 751 residual occurrences are not a deduplicated master count.

The physical parent alone completes in this full CLI campaign at 93.9 s elapsed
with the expected **310 rules and 29 residuals**, matching the preceding
standalone counts. That event is not a saved equation comparison or the timing
of an isolated sector. The standalone exact output comparison is documented in
[the selected-sector study](five_loop_selected_sector_profile.md). Neither
result establishes full-family completion, cold application or certification.

No CPU or allocation profile was taken in this attempt. The source audit finds
that the current path retains every completed exact solution until generation
finishes, then overlaps those solutions with native encoding buffers. The run
does not separate that cost from active worker frames or native global state.
Therefore saving completed sectors may improve retention and restart behavior,
but has **not** been shown to cure this allocation failure. A small follow-up
releases each solution during final encoding. Its independent source audit,
94 application unit tests, 68 integration tests and 36 fresh-extension Python
tests pass; six further Python candidate tests also agree with the frozen
previous CLI. No peak-RSS improvement has yet been measured. Evidence is in
`TMP/candidate-encoding-release.AxS0ih/`. An optional native checkpoint
path now passes 114 application unit tests, 72 integration tests and 38 Python
tests, including deterministic assembly, partial/complete resume, fresh parallel
generation, cold certification/application and failure-path gates. These tests
do not measure five-loop memory savings or establish five-loop completion.
Evidence: `TMP/candidate-checkpoint-release-fixed.Fkhysy/`. An additional K6
full-family save/resume/certification/cold-application gate passes in all four
modes; its [phase measurements](../generation_and_certification.md#checkpoint-k6-release-smoke)
are separate from these failed five-loop campaigns.

This is a different policy/backend and CPU placement from the depth-two runs
below, not a controlled speed ratio. Compilation, input/binary copying and hash
checks are outside timing; no artifact, pivot trace or oracle is reused.
Evidence: `TMP/five-loop-depth-zero-campaign.CWGK0e/`, including the frozen
launcher, raw resource/progress logs and independent `final-result-audit.md`.
The second Möbius-ladder input uses the same frozen CLI and policy, but ends at
its wall deadline without saved output, as recorded next. Neither full physical
family has completed, and the different topologies are not a backend comparison.

## Physical Möbius-eight: bounded factorized depth-zero run, 20 September 2026

This single full-family attempt **reaches its declared timeout**, returning
status 124 without a candidate bundle or final report. It uses the exact frozen
`2627a537` CLI/hash above and unchanged external
[`five_loop_mobius8.toml`](../../examples/input/five_loop_mobius8.toml), SHA-256
`1baf9dfc388c7c1def6df383493a63578c5e53e21d200ea8203e6388aff521d1`.
The request explicitly restricts auxiliary slots 12,13,14 to nonpositive powers,
giving physical root `111111111111000`. Natural ordering, six workers,
`sparse-factorized`, numerical depth zero and all default algebra/output limits
are preserved. No rules, oracle or certified artifact are reused.

Affinity is CPUs 88–93 (six distinct physical cores on NUMA node 2), separately
from the cube's 100–105; this does not eliminate shared-host contention or bind
memory to a NUMA node. Nested pools are capped at one while the sector executor
retains six workers. The predeclared policy sends TERM at 1,800 seconds, then
KILL after up to ten seconds, with the same 32-GiB virtual-address cap.

| Whole bounded attempt | Measured outcome |
|---|---:|
| Launcher / GNU-time exit status | **124 / 124** |
| Wall time | **1,801.81 s** |
| User / system CPU | 10,568.14 / 46.07 s |
| Peak RSS | 17,063,328 KiB (**16.27 GiB**) |
| Swaps reported for this command | 0 |
| Preparation event | 22.0 s |
| Scheduled sectors / scoped zero sectors | 2,686 / 1,410 |
| Global zero proofs (different scope) | 4,480 |
| Completed-sector events | **11 of 2,686** |
| Rules / finite-residual occurrences in those events | 2,361 / 116 |
| Saved bundle / final report | **Neither produced** |

The physical parent (mask 4095) emits a completion event at 1,187.5 s with
**306 rules and 57 finite residuals**. This is not an isolated parent timing or
a saved exact equation comparison. The last sector completion is at 1,294.4 s,
and ordinary solving progress continues to 1,799.7 s. All eleven completion
masks are unique. The residual occurrences do not establish a minimal or
independent master basis.

No allocation failure, panic or other error diagnostic appears in this run's
progress log; the observed terminal condition is the timeout. The enclosing
launcher, GNU time, timeout and solver handles are all absent afterward, and
input/executable/protocol hashes still match. CLI stdout is empty; there is no
saved program to inspect or apply cold. No restart or limit change was made.
Neither sector-completion telemetry nor the parent's result establishes full
family coverage, a closing artifact, or eventual success with larger budgets.

Whole timing includes preparation, all workers, and timeout teardown; copying,
compilation and hash checks are excluded. RSS does not separate accumulated
solutions from live solver frames. No profile or backend comparison was run.
The different cube/Möbius topologies, completed work and CPU placement preclude
a speed ratio; both are incomplete whole-family outcomes. The frozen command,
raw logs, eleven-sector census and independent `final-result-audit.md` are in
`TMP/five-loop-mobius-depth-zero.sVP7ZX/`.

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
`TMP/five-loop-candidate-next.718HJ6/`. At that historical checkpoint both
generation processes were stopped and no further full-family campaign had
been launched; the later depth-zero attempts are recorded above. These unfinished five-loop
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

The narrow follow-up re-solves only that supplied coordinate case using the
existing `SparseTargetOnly` option. The full 997-source trace, 1,489 RHS
coefficients/contexts and three guard branches agree exactly with sparse.
Observed exact lifting falls from **14.637 to 2.971 s**; this is one
single-case diagnostic, not a direct frozen-frame replay, default change or
full-family speedup. No new algebra or validation weakening is introduced. Native
reconstruction already caches complete row images across coefficients at the
same prime and point. Further optimization should target measured remaining
costs rather than duplicate that cache.

See [the detailed selected-sector report](five_loop_selected_sector_profile.md)
for complete boundaries, resources, per-symbol counts and qualifications.
Evidence and independent audit: `TMP/five-loop-sector-profile.Ijiilz/`.
All owned diagnostic clients are stopped; no further generation job was started.
