# Finite-demand rank-extension hot-path measurements

These are optimized, process-isolated controls for the finite-demand engine,
not a completed full five-loop rank-five or rank-six campaign. The production
campaign was neither modified nor restarted by these measurements. No IBPs
were regenerated: every solve uses the existing saved owner programs.

Evidence is in `TMP/finite-metadata-pilots-20261018-gEuF27/`. The steering is
`pilot.py`; every completed run has a guarded process receipt, native result,
events, and fresh-process metadata inspection. Timings exclude compilation.
The comparison uses 32 workers pinned to physical CPUs 200–231, disjoint from
the production campaigns. Other host activity remains possible. The research
guard is 100 GB with a cooperative 20-minute stop and orderly shutdown budget;
it does not alter production limits.

## Implemented changes

- Borrowed native resident-cache probes eliminate temporary powers/owner vectors
  while preserving exact identity, padding and support checks.
- Optional persistent workers and separate work/resource wakeups retain the
  shared descendant queue and deduplication. Scoped joins, reentry refusal and
  nested-Rayon protection prevent suspended-worker deadlocks.
- Optional parallel window materialization uses that same pool; planning,
  cursor order, target admission and atomic commits remain unchanged.
- Private immutable metadata validation is reused only for the same generation
  and structural grants. Changed grants and cold loads retain full checks.
- Encoder-owned manifests can avoid an unnecessary decode round trip when a
  conservative bound proves readability under the existing allocation budget.
- Exact borrowed digest membership speeds Work-directory validation, with the
  original tree as a tight-budget/allocation-failure fallback.
- Immutable manifests use checked sizing plus Bincode's bounded slice encoder.
  General/stateful encoders keep their original single-pass implementation.
- Committed phase timings include retired-metadata cleanup; resident statistics
  refresh at each commit. Both campaign engines remain supported.

All changes are topology/loop-count generic. The checkpoint bytes and original
window partition are preserved; no production ledger conversion is required.

## Frozen work and comparison boundaries

- **Four-loop controls:** every saved literal owner sector in FG, BMW, H and X,
  with its first physical denominator raised from one to two. Each request is
  repeated once. There are 900 distinct sectors and 1,800 input occurrences.
  This is a limited scalar/dotted control, not the full physics numerator
  envelope. Completed Work counts are respectively 6,301, 8,913, 49,423 and
  147,428.
- **Five-loop reuse control:** owner `000011001001011`, six positive slots and
  nine numerator slots, restricted to R≤1, D=10, A≤16, retaining all 67 saved
  owners and routes. Its 2,394 distinct starting integrals are repeated in 32
  required query rows: 76,608 input occurrences, 480 existing prefix windows,
  and 41,055 completed Work records.
- **Five-loop higher-rank control:** the same owner, one numerator coordinate
  taking values 0 through 5, every compatible positive-power composition at
  D=10, and the remaining numerator coordinates zero. This gives 4,921
  concrete inputs, represented as supported fully fixed singleton queries.
  Its completed graph contains 2,021,866 Work records.

“Group size” counts pre-existing deterministic prefix windows, not starting
integrals. A singleton control contributes one integral per window, whereas
production windows can contain hundreds. Equal group sizes do not therefore
imply equal memory use or interruption cost across these workloads.

The operational-setting matrix below used the same Phase-B binary throughout:
SHA256 `4fcc05e98d204713836220efc7dc1d630ec3fd64fdb06a1abb891a191fa740eb`.
It includes typed metadata reuse and bounded writer readability optimization,
but predates the final borrowed digest index and sized-manifest encoder.
It must not be presented as a timing of the final consolidated executable.

## Reusing workers and preparing windows in parallel

For the five-loop reuse control:

| Workers / preparation | Windows per group | Groups | Materialization | Trace | Commit | All group work | Whole process |
|---|---:|---:|---:|---:|---:|---:|---:|
| Per-group pool / serial | 64 | 8 | 0.2351 s | 0.4142 s | 0.0882 s | 0.7850 s | 145.159 s |
| Persistent pool / serial | 64 | 8 | 0.2035 s | 0.3998 s | 0.0903 s | 0.7418 s | 147.710 s |
| Persistent pool / parallel | 64 | 8 | 0.0158 s | 0.4015 s | 0.1053 s | 0.5577 s | 143.788 s |
| Persistent pool / parallel | 256 | 2 | 0.0070 s | 0.3951 s | 0.0911 s | 0.5275 s | 145.226 s |

All four settings completed the same 32 requests, 76,608 occurrences and
41,055 Work records. The phase timers include their named work; “all group
work” additionally includes planning, binding, staging and per-group cleanup.

Parallel preparation reduced materialization by about 12.9× relative to the
persistent/serial arm, and total group work by 24.8%. **This is not a 24.8%
whole-campaign speedup:** these logical-cold runs spent approximately
138–142 seconds preparing the saved programs, followed by approximately four
seconds of final cleanup. Whole-process differences are small and affected by
host variation. Persistent workers alone did not demonstrate a large gain.

Four-loop H/X pool ablations were likewise approximately neutral in wall
time. A first slow persistent-H observation did not reproduce; retain the
repeat instead of interpreting the first observation as a proven regression.

## Higher-rank test of wider groups

The admitted 4,921-point R≤5 control completed with both group sizes:

| Setting | Groups | Trace | Commit | All group work | Whole process | Peak RSS |
|---|---:|---:|---:|---:|---:|---:|
| Persistent/parallel, G64 | 77 | 18.844 s | 5.557 s | 25.688 s | 170.962 s | 8.348 GB |
| Persistent/parallel, G256 | 20 | 19.073 s | 4.871 s | 25.068 s | 169.838 s | 8.350 GB |

Both closed all 4,921 requests and produced 2,021,866 completed Work records;
both passed cold inspection. G256 reduced commit time, but total group time
fell only 2.4%, with no meaningful change in peak memory. This does not
establish G256 as a generally superior setting for production's much larger
multi-point windows.

An earlier attempt represented this subset as one anisotropic coordinate box.
It was explicitly rejected with `UnsupportedGeometry: coordinate box differs
from whole envelope`, after preparation but before any Work was committed.
The corrected input enumerates exactly the same 4,921 points as fully fixed
queries, which the existing API supports. No geometry handling or admission
checks were weakened to make the test pass. The negative receipt is retained
under `phaseB-axis-r5-...`; successful controls use `phaseB-axis-points-...`.

## Exact output checks

Across 26 completed matched G64 controls, the old build, Phase-A/B
operational arms and consolidated candidate produced **byte-identical Work-page and raw-census object
maps**, not merely matching counts. Input selection and query bytes were also
compared. The receipt is `g64-exact-object-comparison.json` and its reproducer
is `verify_g64_objects.py`.

G256 changes page grouping, so byte identity is not required. The native
regressions separately compare exact Work membership, terminals, zeros and
selected-rule multiplicities across group sizes, worker counts and cold
resumes. They also cover publication and cold-loaded exact coefficient
application, plus cancellation and refused commits leaving every grouped
cursor at the previous durable generation.

## Consolidated candidate smoke tests

With the final combined implementation and G64/persistent/parallel at 32
workers, FG/BMW/H/X completed in 3.443/4.442/7.489/13.513 seconds, with the
same exact Work objects as the earlier build. The admitted 4,921-point
higher-rank control completed in 170.963 seconds (25.391 seconds of grouped
work, including 5.229 seconds of commits), versus the intermediate build's
170.962 seconds. Its 1,059 Work-page and 450 raw-page objects were byte-identical.
This small-history control establishes compatibility, not a large wall-time win.

At 64 workers on CPUs 192–255, H/X completed in 7.421/13.460 seconds, with
byte-identical outputs to 32 workers. Grouped work took 0.558/1.696 seconds,
versus 0.521/1.595 seconds at 32 workers: no proportional scaling claim follows.
The new worker-pool configuration was not benchmarked at 150 workers while the
production campaigns occupied the other physical cores.

A separate public CLI gate created a checkpoint with the previous executable,
then resumed it with **persistent workers and parallel preparation explicitly
enabled** in the new executable. A no-op resume preserved CURRENT; an appended
overlapping request completed 2,600 input occurrences and 2,800 Work records.
Publication and cold inspection passed with one rule/one terminal, no unused
resident Work hydration, and unchanged saved RAM policy and CURRENT during
publication. Evidence is
`TMP/finite-cleanup-pilots-20261017-iAwYkn/finalD-parallel-resume-append-publication/`.

## Digest-index microbenchmarks and the rejected raw-map shortcut

The profiled Work-directory uniqueness check does not need sorted iteration.
A standard-library optimized microbenchmark, on CPU 240 with three samples,
compared exact 32-byte membership at 298,884 distinct digests:

| Membership structure | Median time |
|---|---:|
| Owned-key BTreeSet | 0.1199 s |
| Borrowed-key HashSet, pre-reserved | 0.0329 s |

At one million digests the corresponding medians were 0.5290 and 0.1786 s.
The final implementation uses the already-pinned hashbrown library with exact
full-key equality, fallible pre-reservation and public allocation accounting.
Randomized table iteration is never observed. Tight allocation grants or a
declined optional contiguous allocation retain the old tree path; existing
metadata charges, validation order and cold checks remain.

The analogous raw-census map is **not changed**: unlike Work pages, it may
contain many identical repeated references. At 300,000 references/30,000
unique digests, pre-reserved hashing improved the median only from 0.0516 to
0.0478 seconds while increasing observed process memory. With one unique
digest repeated throughout, hashing took 0.0085 seconds versus 0.0041 seconds
for the tree. Over-reserving for all raw references is therefore not a
systematic improvement. Evidence: `digest-membership-results.json` and
`digest-duplicate-results.json`.

## Large-history and encoder measurements

The optimized synthetic metadata control uses 149,442 historical runs. Three
reader constructions requiring full validation took 0.1288 s; reusing their
private immutable validation result took below 1 µs in that control. This
isolates repeated metadata checks, not checkpoint cold loading or IBP work.

A representative 54.37 MB manifest exposed encoding as another remaining
cost. Six alternating optimized runs of the same shape measured a median
188.6 ms with the prior bounded writer and 85.0 ms with checked sizing and
bounded slice encoding, with identical framed bytes. The actual library's
35.69 MB integration microbenchmark also passed byte-identity checks and
measured 159–177 ms versus approximately 62 ms. These are encoding-kernel
measurements, not whole-campaign speedups. Evidence is under
`TMP/bincode-encoder-audit-H4BQ0a/` and
`TMP/rank6-sized-tests-frozen-20261018/`.

## Recommendation and limits

Use **G64, persistent workers and parallel window preparation** as the
conservative tested setting; keep per-group workers, serial preparation and
G256 selectable without rebuilding. G256 remains an experiment, not the
default recommendation. Interruption discards the unfinished group, while
previous groups remain durable; increasing group size increases the potential
amount of work lost to an interruption.

The measurements support reduced per-group overhead and faster preparation,
not a promised rank-six runtime or saturation of 150 cores. Source preparation
is heavily amortized in a long extension. More importantly, the large
production history has metadata costs that these small native controls do not
reproduce; the separate optimized 149,442-run metadata benchmarks address that
specific cost. Full historical manifest encoding remains linear in directory
size; no persistent-tree/on-disk format rewrite is claimed here.
