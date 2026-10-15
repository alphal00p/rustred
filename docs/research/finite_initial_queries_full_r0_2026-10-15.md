# Finite initial queries: complete five-loop scalar control

## Decision

The unchanged 116-query R0/D9–D10 control closes and passes independent cold
reinspection with both ordinary traversal and opt-in finite-v3 traversal.
**The default finite-v3 configuration is slower on this complete control.**
A single explicit aggregate-budget correction removes its expensive fallback,
but does not demonstrate a useful whole-pipeline speedup. Keep finite-v3 opt-in;
do not recommend replacing the user's production campaign on this evidence.

This negative result complements, rather than invalidates, the smaller
[four-parent controls](finite_initial_queries_controls_2026-10-15.md) and
[279-point R1 publication](finite_demand_publication_2026-10-15.md). Those
exercise cases where finite traversal is useful. They do not justify applying
it indiscriminately to every small starting region.

## Frozen workload and timing boundary

The exact original input is
`campaigns/five-loop-minimal-r0-d10-incremental/inputs/queries.json`:
116 required requests, no auxiliary helpers, and 92 distinct admitted roots
after exact containment aliases. Both arms use the same 69 prepared owner
payloads and existing selection, without new rules, new helpers, changed
query bounds or production writes.

Frozen optimized CLI:
`TMP/releases/20261015-finite-initial-queries-retry/rustred`

SHA256: `92490ebf50f32014ae2978e18c95de4fe3638ada7185b9d0a54f0edc00d836d1`.

The matched pair used four workers on physical CPUs 16–19, a 32 GB process-tree
RSS ceiling, and a 150 GB host reserve. Each phase had a 600-second cooperative
deadline, with a 1,440-second overall allowance and 120 seconds for safe cleanup.
No deadline or memory stop occurred. Compilation is excluded. The subsequent
candidate-only correction used the same executable and resource envelope.
Shared-host and filesystem contention were not eliminated; process startup
does not imply a cold filesystem cache.

Each pipeline is a walk followed by independent native `All/Off` cold
reinspection. Publication is **not** part of these timings. The native walk
includes preparation; traversal is the separately reported post-preparation
interval. Guarded totals additionally include supervision and process draining.

## Measurements

All values are seconds except RSS. The corrected candidate is a later run,
not a second matched pair or a replicated speedup measurement.

| Metric | Ordinary | Finite-v3 defaults | Finite-v3 explicit aggregate allowance |
|---|---:|---:|---:|
| Native preparation | 127.829 | 129.530 | 143.289 |
| Native traversal | 43.910 | 79.382 | 40.475 |
| Native complete walk | 171.738 | 208.912 | 183.764 |
| Native cold preparation | 120.248 | 125.134 | 128.426 |
| Native cold reinspection | 25.213 | 53.029 | 46.826 |
| Guarded walk + cold verification | 333.059 | 402.959 | 378.910 |
| Peak sampled process-tree RSS, decimal GB | 8.512 | 8.436 | 7.762 |

The default candidate is about 21% slower over walk plus cold verification.
Its native traversal is about 81% slower. The corrected candidate's traversal
is about 8% below the original baseline, but its walk-plus-cold total is about
14% above it. Preparation varied, and cold replay re-executes cases separately
and may repeat shared subproblems. Neither candidate demonstrates a
full-control deployment win.

The original pair completed in 738.181 seconds including both arms and driver
overhead; the separate correction completed in 380.102 seconds. These driver
totals differ slightly from summed supervised phase timings above.

## What caused the default regression?

The default candidate made 92 attempts: 80 finite completions, 12 declines,
and no failed cases. Eleven declines were empty R0 intersections. Their
parents have too many positive lines to satisfy this particular starting
R0/D≤10 intersection; ordinary processing correctly handles them as empty.
They remain recorded requests and are not removed from future extensions.

The remaining decline was a **nonempty nine-point request**, native root 60,
owner `101011111001010`. Its finite trace reached the configured aggregate
endpoint allowance: 4,001,967 requested versus 4,000,000 allowed. At that point
it had spent 10.566 seconds, completed 254,949 states, and still had 3,870
queued nodes. There was no missing rule or owner. It then fell back to the
ordinary symbolic traversal, repeating some effort.

The failed trace alone does not explain the entire 35.472-second traversal
regression. The ordinary fallback cone is large, and separate finite cases do
not share their completed point maps. Coordinator merge stages were slightly
cheaper; inspector/ordered-prefix waiting increased instead. Consequently,
neither a coordinator micro-optimization nor a smaller CP6 record count is an
adequate explanation or performance claim.

The corrected diagnostic explicitly raised the six aggregate finite-trace
allowances and three aggregate reduction allowances to their accepted maximum.
It did **not** change the seed limit (1,024), positive-layer limit (64), seed
buffer (1 MiB), per-operation algebra limits, mathematical scope, actual RSS
guard or pilot deadline. No default was changed. It completed all 81 nonempty
finite roots, leaving only the 11 empty declines; all 116 requests then passed
cold reinspection. This establishes that the refusal was an aggregate budget
boundary, not evidence of a missing recurrence.

Independent audit measured root 60's completed trace at 289,698 operational
states, 289,319 physical keys, 116,211 rule applications, 81,989 transports,
18,502,196 transport operations and 4,652,550 endpoints. It took 11.463 seconds
live and 12.557 seconds in cold replay. Across all 81 completed cases, only
this endpoint count exceeded an original default aggregate allowance; all
other observed counters fit their old defaults. Thus the diagnostic supports
an explicit bounded endpoint allowance for this fixture, not globally
removing limits. No further cap-tuning run is planned.

The corrected checkpoint contains 92 summarized records rather than the
baseline's 167,591 domains. That is a representation change, **not** a
1,800-fold work reduction: each finite record encapsulates many actual routed
integral visits. Diagnostic counts are not globally unique across cases.

## Evidence, limitations and next discriminating test

Matched pair:
`TMP/finite-v3-full-r0-pair1-20261015/`.

Candidate-only correction:
`TMP/finite-v3-full-r0-resource-correction-20261015/`.

Drivers and exact commands:
`TMP/finite-initial-queries-real-plan-20261015/{full_r0_pair.py,full_r0_correction.py}`.

Each evidence directory retains commands, immutable-input hashes, native
results, guards and cold-verification reports. Standard read-only `strings`
extraction exposed the saved diagnostic refusal; that is supporting forensics,
not a new binary decoder or a substitute for the native verifier. Both cold
runs verify all 92 roots and all 116 recorded requests, with zero queue and
frontiers and unchanged owner payloads. `family_closure_claim` remains false:
this is scoped scalar closure, not unrestricted five-loop closure or R6 coverage.

Do not perform more blind cap tuning. A cross-query memo would need a measured
overlap benefit, exact owner/order binding, safe completed-only reuse,
dependency/census provenance and replay validation; it is not authorized by
the 92-record count alone. The 81 separate traces sum 1,211,505 physical-key
visits and 1,215,215 operational states; the available receipts do not identify
the globally unique subset. The smallest next experiment is one diagnostic
native trace of the same exact union of starting points, using the existing
union-aware admission and multi-target traversal APIs. Compare its complete
terminal/frontier sets and work against the per-case union before designing
cross-case persistence. It must not replace the union by a rectangular hull.
A mixed eligible/ineligible query control is staged separately but deferred.
The other active
performance lane measures Symbolica's existing rational-coefficient polynomial
conversion for application, independently of finite traversal.
