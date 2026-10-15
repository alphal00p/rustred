# Shared finite-demand tracing: complete R1 input pilots

## Result and scope

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
