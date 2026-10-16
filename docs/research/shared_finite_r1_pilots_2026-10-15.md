# Shared finite-demand tracing: complete R1 input pilots

## Resident Work index: durable R1 and cold parity passed

The exact resident index passed its 556-test core gate and the combined app's
independently audited 345-test backend/publication component. Four failures in
public inspection remain explicitly excluded from this private performance
gate; the inspector repair is being built separately. These results do not
make the public frontend launch-ready.

The first prefix test retained all 232 R0/R1 declarations and paused at the
same 81 committed windows as the original paged and indexed tests. Native
input/cursor state, generation82, 300,673 completed Work, 2,557 occurrences and
CURRENT SHA256 are identical across all three runs.

| Prefix measurement, four workers | Indexed page lookup | Resident Work index |
|---|---:|---:|
| Full driver | 165.983 s | 140.438 s |
| Preparation | 126.646 s | 120.031 s |
| All window steps | 24.417 s | 4.690 s |
| Native calls plus export, summed | 22.022 s | 3.213 s |
| Commits, summed | 1.239 s | 0.901 s |
| Peak process-tree RSS | 7.992 GB | 8.026 GB |
| Completed-baseline hits | 3,138,307 | 3,138,307 |
| Page-cache hits | 41,837,465 | 0 |
| Backing reads | 3,365 /73.38 MB | 0 |

This is one matched prefix observation, not a replicated whole-campaign timing
claim. The explicit resident grant is 16 GB within the unchanged 32 GB process
guard. Every window used the resident index without fallback. Final estimated
retained payload was 63,555,441 bytes plus 150,994,944 bytes of reserved
coexistence; this estimate is not allocator-exact RSS. The index was initialized
once and updated only after durable commits.

The harder rank-one continuation and subsequent cold census comparison
**completed and passed independent audit** under the same source and policy.
Evidence for the first phase:
`TMP/finite-demand-resident-r1-validation-20261016/phase-a/independent-audit.json`.
Frozen diagnostic source: `22637ccd`; app executable SHA256:
`c31458e6cd630483fc315ef5e34b11c2e484a5a3d0d22ea3e76a937ac8bf3ded`.

Phase B committed the remaining407 windows: all232 declarations exhausted,
60,807 occurrences and6,047,386 completed Work at generation489. Exact
record-by-record comparisons against the accepted in-memory reference passed
for all683 terminal keys,1,110,963 zero keys and5,023 composite selected-rule
identities, including multiplicities (2,354,285 rule applications). The fresh
process then reopened the same generation, committed no new windows, and
produced byte-identical raw census streams. This is not merely count equality.

| Completed continuation measurement | Seconds |
|---|---:|
| Prepare source, authenticate scope and restore | 119.147 |
| All407 window steps, including durable advancement | 148.104 |
| Native calls plus export, summed | 125.430 |
| Commits, summed | 15.501 |
| Final raw census stream | 1.578 |
| Native process plus exact output comparison | 280.775 |
| Additional fresh-process cold reopen and comparison | 140.789 |
| Complete guarded Phase-B driver | 430.878 |

Peak process-tree RSS was10.752GB. All407 native calls used the resident index,
with zero Work-page reads/cache probes and no fallback. Cold hydration did
read the saved Work pages: the zero counters refer to native-window lookup,
not zero total disk IO. The final estimated resident payload was1.172GB plus
151MB reserved coexistence. The checkpoint contains4,997 files /302,314,985
bytes, including3,381 Work pages and636 raw census pages. Its largest manifest
is333,940 bytes;484 nonempty Work runs are inferred from the native delta log.

The same row118 call/export interval fell from68.372s (indexed pages) to4.946s
(resident Work), while the formerly censored hard row156 completed in112.924s.
The old paged/indexed runs did not complete the full continuation within their
30-minute inclusive pilots, so there is no completed whole-run speedup ratio
against them. Preparation, continuation and cold-open times above are separate;
do not present430.878s as solver-only time or as a complete new R6 campaign.

Full continuation audit:
`TMP/finite-demand-resident-r1-validation-20261016/phase-b/independent-audit.json`.
Public lifecycle readiness, artifact publication through that frontend,
coefficient application and higher-rank closure remain separate gates.

## October 16: indexed persistence and higher-rank diagnostic

The indexed page LRU passed its 35-test IO gate and the combined binary's
finite-component gate (173 executions / 172 distinct tests). The latter is
not approval of the full publication adapter: a separate new fixture setup
failed, was corrected, and awaits its next native run. The matched tracing
pilots below used the frozen finite component, not an untested production CLI.

| Same-source durable measurement, four workers | Original page cache | Indexed page cache |
|---|---:|---:|
| R0-prefix full driver | 160.301 s | 165.983 s |
| R0-prefix preparation | 114.182 s | 126.646 s |
| R0-prefix native calls plus export | 29.700 s | 22.022 s |
| R0-prefix retained Work | 300,673 | 300,673 |
| R1 continuation completed windows | 171 | 171 |
| R1 continuation completed call/export sum | 380.453 s | 163.855 s |
| Matched row118 call/export | 129.469 s | 68.372 s |
| R1 continuation complete driver, censored | 1,632.754 s | 1,635.016 s |
| R1 continuation peak RSS | 8.385 GB | 8.474 GB |

The same 171 continuation windows have identical input counts, new Work,
baseline hits and page IO counts. Both checkpoints end at generation253 /
1,250,965 Work /26,161 input occurrences, with CURRENT digest
`20ee1589529d1dbe13dd4bf7410ada753a6ea7cd1be9b48c22225818d4b371d7`.
The index cuts row118's measured call-plus-export interval by approximately
47%, but **neither
durable run completes R1** within the pilot. Indexed row156 reached 2,261,050
scheduled states and 1,950,895 local expansions before cancellation; none of
that unfinished window entered the committed census. Comparing this unequal
partial progress with the baseline is not a full-window speedup measurement.

There was no RAM stop or observed missing rule/owner. Both owned process groups
drained. The remaining bottleneck is repeated exact lookup through overlapping
runs, even with constant-time page-cache indexing. A bounded resident exact
Work index is the next candidate, not an implemented or proven optimization.
Evidence: `TMP/finite-demand-indexed-r1-validation-20261015/phase-a/` and
`phase-b/`, with independently audited native receipts. Full publication and
fresh-process full-R1 census comparison remain pending.

A separate higher-rank diagnostic kept the accepted **in-memory** R1 binary
and policy, appending the actual second amendment without changing its bounds.
All 348 cumulative declarations passed native geometry/count preflight:
701,972 occurrences, 54 empty rows. Its planned 1,620-second stop produced an
inclusive 1,653.228-second censored run, peaking at 16.568 GB. The independent
audit accepts only the completed prefix:

- 253/348 rows (all 232 R0/R1 declarations and 21 amendment-two rows);
- 1,330 complete windows /202,630 occurrences, including 141,823 from R2;
- 23,799,934 retained Work, 862 terminals and 4,348,444 zero keys;
- 6,128 selected-rule identities /10,108,567 rule applications.

The next row's six-input window was cancelled after 328.600 seconds. Its
5,298,918 scheduled states and 4,840,354 local expansions are excluded from
those completed totals. No missing rule/owner or memory stop was observed.
The ordinary full-result postverifier did not run after wrapper termination;
the independent read-only audit checked source bindings, all input geometries,
the completed prefix and cancellation accounting. This JSON evidence is not a
resumable Work checkpoint, full R2 closure, coefficient reduction or an R6 ETA.
Compact receipt:
`TMP/full-r2-resource-governed-20261015/runtime/independent-audit.json`.

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
The same-source fresh-process R1 continuation **hit its planned pilot deadline**,
not a missing-rule or memory failure. Its independent audit is
`TMP/finite-demand-durable-r1-validation-20261015/phase-b/independent-audit.json`.
It committed171 additional windows, reaching generation253 /1,250,965 Work
records /26,161 input occurrences. Row156/window0 was cancelled and not
committed. The last completed generation remains available; no fresh native
reopen of that final generation is claimed yet.

The 1620-second timer requested SIGTERM; all owned processes drained, and
inclusive time was1632.754s with8.385GB peak RSS. The run performed1,049,117,546
cached-page probes but read424.5MB from disk, with no page evictions. The hard
window alone consumed1098.196s before cancellation:699,479 local expansions
out of984,639 scheduled nodes,693,893,718 page-cache probes,29.69MB read. Those
local expansion counters are not recursively closed work and were not added to
the retained baseline. The full raw-census comparison and second fresh-process
cold-open phase were **not run**. This is a censored performance result, not
complete durable R1, a mathematical obstruction, or a reason to raise a cap.

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
cache mutex, on every hit. The isolated indexed-page-LRU slice `c010d50c` passed
35 focused IO tests, including three new eviction/index/budget controls, and
independent source/runtime review. Its normal combined app check also passed;
the subsequent finite native gate and matched measurements are recorded above.
It does not change the captured generation, format, lookup result, scope or
byte budget. The completed fixed baseline was never modified. Further positive
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
