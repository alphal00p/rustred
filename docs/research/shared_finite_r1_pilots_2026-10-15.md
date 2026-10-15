# Shared finite-demand tracing: complete R1 input pilots

## Durable follow-up: completed prefix and a measured lookup bottleneck

The separate paged backend passed its 164-execution native gate and a real
planned pause after the original R0 prefix. It retained all232 R0/R1 query
declarations and durably saved81 windows /2,557 occurrences /300,673 Work
records at generation82. The result is correctly **paused**, not complete R1.
There are131 exhausted rows: the116 originals plus15 already empty R1 rows.

| Durable prefix measurement, four workers | Result |
|---|---:|
| Full driver including preparation/checks | 160.301 s |
| Preparation / exact scope construction | 114.182 s |
| All steps | 31.576 s |
| Native calls including delta export, summed | 29.700 s |
| Commits, summed | 1.106 s |
| Peak process-tree RSS | 8.017 GB |
| Checkpoint file bytes | 15,147,445 |
| Backing page reads | 3,365 / 73.38 MB |
| Cached-page probes | 41,837,465 |
| Completed-baseline hits | 3,138,307 |
| Page evictions | 0 |

Independent audit passed at
`TMP/finite-demand-durable-r1-validation-20261015/phase-a/independent-audit.json`.
The same-source fresh-process R1 continuation is running under its separate
30-minute inclusive guard; no full durable R1 result is claimed yet.

The continuation exposes a CPU-side lookup cost that the in-memory result did
not measure. For the same51-start row118, both versions close503,501 new Work
states, apply239,050 rules, visit6,045,804 transport endpoints and reuse4,559,042
completed states. The in-memory final native snapshot is4.670s; the durable
final native snapshot is128.753s, with129.469s for the enclosing native call
plus export and0.970s for its commit. The durable call performs153,004,129
cached-page probes but only230 reads /7.139MB, with no evictions. This is a
specific matched-work diagnostic, not a paired whole-campaign timing ratio.

The current directory checks overlapping immutable runs. Its page cache also
does linear digest lookup and movement in a `VecDeque`, under its existing
cache mutex, on every hit. An isolated indexed-page-LRU implementation is now
being tested, without changing the captured generation, format, lookup result,
scope or byte budget. The fixed baseline continues unchanged. Further positive
or complete-resident Work membership caching remains a separate design option,
not an implemented speedup. Negative probes also contribute to the page counts;
dividing page probes by positive baseline hits is not an exclusive positive-hit
cost measurement.

## Follow-up: full cumulative R0+R1 trace completed

A fourth, explicitly resource-governed diagnostic **completed all 232 unchanged
declarations**, including the four-start case that stopped the earlier pilots.
The original request/amendment chain was authenticated before changing an
isolated copy of the aggregate execution allowances. No source geometry,
owner/rule/routing policy, per-expression algebra/expansion limit or production
campaign was changed.

The six aggregate work counters and three prepared reduction aggregate counters
were made nonbinding together, with checked overflow still enforced. The
512-point windows, census/retention byte allowances, 32 GB RSS guard, 150 GB host
reserve and 30-minute inclusive ceiling remained unchanged. This was a fresh
in-memory diagnostic, not reuse of a prior checkpoint or a larger-limit retry
of a live production process.

| Measurement | Completed shared trace, four workers |
|---|---:|
| Required query declarations | 232 / 232 |
| Starting occurrences, including overlap | 60,807 |
| Complete windows | 488 |
| New operational states | 6,047,386 |
| Rule applications | 2,354,285 |
| Completed-baseline hits | 252,397,738 |
| Raw terminal keys | 683 |
| Raw zero keys | 1,110,963 |
| Composite selected-rule identities | 5,023 |
| Source preparation | 112.847 s |
| Native batch snapshot time, summed | 126.068 s |
| API calls including completed-state retention | 233.387 s |
| Whole trace side | 239.712 s |
| Complete driver, including preparation and output checks | 371.565 s |
| Peak process-tree RSS | 10.144 GB |

The guarded native process and output verifier both exited successfully, with
all process groups drained and no resource stop. Every row exhausted its exact
native count; complete census multiplicities match rule-application counts.
The recorded input union has 58,250 points after removing R0/R1 overlap. No
global distinct-start counter is inferred from the occurrence counter.

The formerly blocked four-start row completed 4,711,319 operational states,
525,244,721 routed endpoints and 1,710,456,345 aggregate operation charges in
111.304 seconds of native batch time. This confirms that its earlier aggregate
refusals were not missing-rule failures. It does not imply that every later
rank can finish within the same resources.

This is **complete finite-scope tracing**, not a durable saved-campaign artifact,
an independent mathematical replay of every IBP, coefficient back-substitution,
a minimal master basis or full R6 closure. There is no completed matched
separate-window run for a full-R1 timing ratio. The previously measured 89.8%
work saving remains a comparison of the identical completed prefix only.

The API time exceeds native snapshot time by about 107.3 seconds. That establishes
material overhead outside the batch scheduler interval; retention/merge/export
and teardown are plausible contributors, not separately profiled percentages.
In particular, the in-memory implementation stages cumulative census metadata
at every window. The new disk backend must not multiply that cost with repeated
cold validation or page-cache reconstruction.

This workload also exposes a concrete persistence sizing issue: the zero-key
coordinates alone need 133,315,560 uncompressed bytes, before vector/index
overhead. The first durable slice's 64 MiB state/decoder bound cannot store the
full R1 census. It must refuse safely until a bounded, properly sized or paged
census solution is implemented; compression alone does not address decoded RAM.
The native disk/cursor correctness gates remain useful but do not establish
that a full R1 durable run fits that first format.

Evidence: `TMP/full-r1-resource-governed-20261015/runtime/`, including
`summary.json`, `verified.json`, raw events and `native/result.json`.
Frozen optimized binary SHA256:
`791c8276925a153e9bd69bb77043753a79566c5e975733415ba2d529d4e33624`.
Policy SHA256:
`d1716af7ca576286a68ccac24290cc462412d53d6cb7f60893f407806f08e835`.
The associated 29-test native implementation gate passed before launch.
Independent final review passed the full input/census/completion and timing
interpretation; its receipt is retained beside the runtime evidence.

## Earlier incomplete attempts: result and scope

The new exact finite-demand runner has been exercised on the unchanged
cumulative R0+R1 input: all 116 original declarations and all 116 rank-one
amendments. **None of these three pilots completed R1.** Each stopped at an
aggregate work allowance on the same four-input row, not at a missing rule,
missing owner, RAM guard or time limit.

There is nevertheless a substantial measured improvement on the identical
completed prefix. Sharing previously closed reduction graphs reduced new
operational states from **12,269,817 to 1,250,965 (89.8%)**, and rule
applications from **5,685,673 to 529,893 (90.7%)**. Both prefixes contain exactly
the same 549 raw terminal keys, 169,488 zero keys and 3,739 composite selected-rule
identities. These are prefix results, not a whole-campaign speedup or R6 closure.

The runner source is the independently reviewed local commit `7ef85e0b`,
compiled with the already tested core/prefix slices as `9f4a613d`. The frozen
optimized app test executable has SHA256
`e09c31cb16551a3657091a1ccd59bbbb4996c3d72ce04856818e47f8d37069d0`.
The runner is an internal input-steered diagnostic, not yet a durable
saved-campaign backend or an artifact publication command.

## Native input accounting

All 232 rows passed the native whole-domain/prefix-envelope equivalence check.
No query, geometry, role, owner payload or route was removed or rewritten.

| Input declarations | Occurrences | Nonempty rows | Empty rows |
|---|---:|---:|---:|
| Original R0 | 2,557 | 81 | 35 |
| R1 amendment | 58,250 | 101 | 15 |
| Cumulative | 60,807 | 182 | 50 |

The R0 regions are contained in their R1 counterparts. The R1 support/D slabs
are disjoint, so the union has 58,250 starting points. This is an input count,
not a bound on descendant work. All 69 prepared payloads retain their order.

## Pilot results

All pilots used four workers on CPUs 16–19, the same optimized executable,
512-point maximum input windows, a 32 GB process-tree RSS guard and a 150 GB
host reserve. Preparation and orderly shutdown were inside the 30-minute
pilot ceiling. Every process group drained normally after its native refusal.
Compilation is excluded from the following measurements.

| Pilot | Stopping resource | Prepared sources (s) | Partial trace side (s) | Entire driver (s) | Peak tree RSS (GB) |
|---|---|---:|---:|---:|---:|
| Separate windows, compare mode | 32M aggregate routed endpoints | 132.767 | 157.619 | 306.394 | 8.203 |
| Shared windows | 32M aggregate routed endpoints | 116.598 | 42.926 | 171.218 | 8.238 |
| Shared, endpoint allowance 128M | 500M aggregate routed operations | 118.266 | 66.523 | 197.366 | 8.520 |

All trace times above are **censored**. The first comparison stopped on its
separate side before reaching its shared side. The subsequent shared runs are
unpaired diagnostic runs, not a completed timing comparison. Each completed
the same first 156 rows and 26,161 starting occurrences; the failing row
committed no window and no starting occurrence.

| Failed-row counters after workers drained | Separate | Shared | Shared, 128M endpoints |
|---|---:|---:|---:|
| Scheduled operational states | 2,129,553 | 1,485,085 | 2,562,618 |
| Locally completed states | 1,734,267 | 1,148,232 | 2,223,410 |
| Queued states | 395,282 | 336,849 | 339,204 |
| Failed local states | 4 | 4 | 4 |
| Rule applications | 633,348 | 382,691 | 762,937 |
| Transport calls | 975,699 | 671,669 | 1,187,652 |
| Aggregate routed-operation charge | 168,290,510 | 156,194,959 | 499,988,931 |

The failed batch of the last pilot recorded 75,911,695 completed-baseline hits, maximum descendant
scalar rank 6 and dot excess 11. Starting R1 does not permit clipping those
descendants. No ETA follows from the queue appearing approximately stable
while more local states complete.

## The four-start bottleneck

The failing zero-based row is 156:
`rank1-d10-00040-40dbed7ceaf16dcf01a1eb5d`.
Its support is `011101111111100`: eleven active propagators, with
`A <= 15`, `D = 10`, `R <= 1`. These constraints force `A = 11`, `R = 1`.
There are exactly four starts: every active power is one, and one inactive
coordinate is minus one, on physical axis 1, 5, 14 or 15.

The corresponding R0 row is empty. Consequently this reduction cone was not
tested by the successful full R0 diagnostic. Four starting integrals can still
generate millions of descendant states; input cardinality is not a measure
of reduction difficulty.

The last refusal requested 500,000,706 aggregate routed operations against
500,000,000. Its final charged count was 499,988,931. Their difference is not
the exact triggering reservation: other in-flight workers can still admit
bounded work before cancellation drains. Transport accounting is conservative
work admission, not a literal count of CPU instructions or CAS operations.

## Interpretation and next decision

These experiments separate two observations:

1. Complete-only sharing removes considerable duplicate traversal on the
   completed prefix, with exact raw-census agreement.
2. The policy's aggregate proxy-work allowances stop a new expensive cone
   well before the physical RAM or pilot wall-time limits.

The explicit 32M-to-128M endpoint correction did not alter local expansion or
algebra limits. It exposed the next aggregate allowance rather than a new
mathematical failure. Further automatic increases are not authorized by this
result. First separate total-work policy from per-expression safeguards and
memory retention, then choose one controlled diagnostic that can distinguish
an artificial admission refusal from genuinely impractical reduction paths.
Neither dropping the row nor treating its unfinished states as masters is
acceptable.

Durable completed-work storage, fast restore, cursor adoption and ordinary
publication remain separate implementation gates. The storage codec has its
own passing native tests; its integration with the core is in progress.
No production campaign, installed binary, source artifact or publication was
modified by these pilots.

## Reproduction evidence

All paths below are local, untracked evidence beneath the workspace:

- Native input counts:
  `TMP/full-r1-native-envelope-preflight-20261015/evidence/native-counts.json`.
- Runner policy/scripts and all three raw runs:
  `TMP/finite-demand-runner-20261015/TMP/full-r1/`.
- Separate run: `runtime/`; shared run: `runtime-shared/`;
  endpoint correction: `runtime-shared-endpoints128m/`.
- The last directory's `independent-audit.json` verifies all 232 source rows,
  69 payloads, unchanged local limits, exact completed-prefix identity and
  the resource-refusal interpretation.
- Runner native gate:
  `TMP/finite-prefix-windows-20261015/TMP/runtime-runner-tests/`:
  24 distinct tests passed (six runner, twelve prefix and six lifecycle),
  none ignored or skipped. Optimized compilation took 1,277.967 seconds;
  the supervised test gate took 8.591 seconds.

Each new experiment must use a new evidence directory and retain these
negative results. Do not overwrite a failed run with a larger-budget retry.
