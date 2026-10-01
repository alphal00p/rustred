# Reusing stale snapshot-negative lookup prefixes

Status, 2026-10-01 03:10 UTC: isolated implementation `c51926be` was integrated
as `f328844f` after independent source review. Native correctness passed:
1,326 unique application/CLI tests, including the ten new targeted tests;
12 pre-existing ignored tests remain ignored. No measured speedup or
production change. The optimized performance build/comparison is still pending.

## Why investigate this

The repaired five-loop run exposes substantial containment-index work on both
the coordinator and inspectors. A short sampled profile motivates this direction
but cannot supply an unbiased sustained cost breakdown. See
[the current measured controls](epoch_p1_compact_2026-10-01.md).

In the completed historical full-A1 h0 pilot:

| Quantity | Observed value |
|---|---:|
| Accepted source rows | 178,474,588 |
| Rows already carrying a stored target | 162,715,550 |
| Targetless rows | 15,759,038 |
| Same-view rechecks skipped | 824,583 |
| Remaining stale or unprobed full-store searches | 14,934,455 |
| Source-resolution wall time | 221.497 s |
| Entire P2 preparation wall time | 312.469 s |
| Native elapsed wall time, including preparation | 1,202.645 s |
| Traversal wall time alone | 1,138.826 s |

Source resolution is 18.42% of native elapsed time (19.45% of traversal), but
includes query construction, hashes, and positive verification as well as
lookup. This is not a lookup-only timer or an achievable speedup. The rows
above do not measure snapshot lag.

Currently, a negative inspector result from an older snapshot causes P2 to
search the current store from ID zero. The Ready implementation already has
the analogous append-watermark optimization, and the shared aggregate index
already accepts a `first_id` lower bound. This proposal reuses that primitive;
it introduces no new algebra or CAS implementation.

## Mechanism and proof boundary

Suppose a worker has searched a snapshot containing IDs `0..w` and found no
container for its successor query. While it works, the coordinator may append
IDs `w..t` and retire old live IDs. It need not repeat the old-prefix search
if all of the following conditions hold:

- Published domain geometry and summaries are immutable.
- Existing live fallback candidates can be removed, but cannot become newly
  eligible during the invocation.
- Quarantine is fixed for that worker session; rescue runs before workers.
- The negative belongs to the actual session, query, snapshot and watermark.
- The worker performed the normal complete lookup, not a synthetic AllMiss
  policy.

Then every old fallback candidate still eligible at commit was eligible in the
snapshot. The honest negative excludes it, so searching only newly appended
IDs preserves the canonical minimum-ID fallback winner.

For example, a negative search of IDs 0 through 999 need only revisit the
general containment index from ID 1000 after eight new domains are appended.
It still performs the following checks over the **whole current store**:

1. Collision-confirmed exact-image lookup, including retired exact aliases.
2. The current dominant-orthant shortcut and its normal priority.
3. Independent exact containment verification of every positive winner.

Only general forward containment gets the lower bound. Summary/digest checks,
quarantine, local deduplication, antichains, dependency publication and strict
coverage obligations are unchanged.

## Minimal integration

Use a private invocation-local context from the native controller's existing
snapshot publication and dispatch session. After existing P1 job checks, carry
an optional eligible negative-prefix bound into P2. Do not reconstruct this
eligibility merely from deserialized `LookupReport` fields.

Unbound, reference, synthetic AllMiss and raw restored stale results do not
receive the new suffix shortcut and retain full lookup. The existing same-view
exact-only shortcut and its validation contract remain unchanged. Resume
reissues unfinished jobs with new session/sequence/version/watermark bindings;
only freshly dispatched jobs may acquire the new eligibility.
No checkpoint schema change, retained per-query proof, extra snapshot-root pin
or authentication framework is intended.

This context is **not a certificate that arbitrary worker bytes are truthful**.
An invented nonexact negative may create redundant unresolved work or different
IDs; the existing same-view shortcut already has that limitation. It cannot
discharge a dependency or certify closure. Positive coverage remains checked.
Exact winner equivalence is required for legitimate worker results, not claimed
for arbitrary forged negative contents.

## Tests and decision rule

- Differential full versus suffix lookup over append/retirement sequences,
  layered snapshots and group reorderings; compare exact hit kind and ID.
- New exact/nonexact containers, retired exact aliases, dominant orthants and
  quarantine exclusions.
- Old-session rejection, unbound fallback, and fresh replay eligibility after
  restore/rescue.
- Invalid metadata, forged negatives, malformed positives and validation-error
  precedence; no false coverage may result.
- Cancellation leaves no partially published plan. Physical scan counters and
  polling frequency may decrease; they need not remain byte-identical.
- Fixed-cut canonical graph/record comparisons, then optimized four-loop and
  representative five-loop controls against the same baseline.

Reject or revise the implementation if old IDs can become newly eligible,
session binding is insufficient, or canonical positive selection changes for
legitimate results. Park it if saved scans do not improve total campaign cost.
CPU utilization alone is not a success criterion.

The earlier frozen old/new compact-row benchmark finished independently;
its results do not validate this later implementation.

### Executed correctness checks

The cached release test build completed in 1,057.516 seconds with the
application package at opt-level 1, followed by 120.175 seconds for the guarded
test sequence. This is a correctness profile, not a performance comparison.
The strict inherited Symbolica license setting was enabled; there were no
license-related skips, compiler errors or guard stops.

| Test group | Passed | Existing ignored |
|---|---:|---:|
| Targeted stale-prefix tests (also included below) | 10 | 0 |
| Full application library | 1,301 | 12 |
| Candidate CLI | 19 | 0 |
| Routed-campaign CLI | 6 | 0 |

The full suite includes real native snapshot-versus-AllMiss equivalence,
rolling interruption/resume in both width directions, durable result-escrow
restore, quarantine refusal, saved-sequence replay and cancellation checks.
The new layered-snapshot test compares actual P3 records, dependencies and
ledger output. The forged-negative test verifies that missing reuse creates
pending work, not false closure. The focused ten are a subset of the 1,326
unique passes, not ten additional unique tests.

Evidence is under `TMP/postlaunch-20260930/stale-negative-prefix/` in
`native-build/` and `native-tests/`. Both owned process groups fully drained.
These checks establish the tested correctness boundary; optimized combined
four-loop and representative five-loop performance evidence remains required.

### Four-loop opportunity limit

The earlier optimized Ready/Epoch ABBA still fails its primary non-regression
gate: Epoch is 9.63% and 9.89% slower. A receipt-only decomposition of the mean
1.386618-second gap is:

| Component | Epoch minus Ready, seconds |
|---|---:|
| Recorded native traversal | +1.316925 |
| Recorded preparation | -0.079670 |
| Internal cold-verifier total | +0.529294 |
| Combined external/untimed residual | -0.379930 |

The residual combines startup, untimed finalization and completion detection;
it is not a measurement of removable polling overhead. The primary boundary
is unchanged, and the consistent traversal penalty is real in these receipts.

Epoch's entire four-loop source-resolution phase averages 0.534137 seconds.
Prefix reuse targets a subset, so its direct savings alone cannot close the
measured fixed-trace gap. The roughly 2.668-second inspect/wait phase includes
oldest-prefix waiting and overlaps worker activity; it is not all removable
overhead. Changed timing might change overlap or the reached work, but that
requires whole-campaign evidence, not an assumed multiplier. The larger
five-loop opportunity must therefore be evaluated separately.

The later P1/compact-row comparison is neutral and is not a replacement for
the Ready control. Local detailed accounting is in
`TMP/postlaunch-20260930/FOUR_LOOP_GAP_COST_BUDGET.txt`.

## Source and evidence pointers

- `walking/epoch/merge.rs`: `resolve_miss` and existing P1 checks.
- `walking/epoch/store.rs`: `lookup_controlled`, exact and orthant priority.
- `walking/queue/index.rs`: `find_from` / `find_controlled(first_id)`.
- `walking/queue/prepared.rs`: Ready's watermark precedent.
- `walking/epoch/dispatch.rs`: fresh replay session binding.
- `walking/epoch/checkpoint/restore/runtime/rescue.rs`: pre-worker amendments.

All source paths above are relative to
`crates/rustred-app/src/application/routed_campaign/`.
Historical timing evidence is in ignored
`TMP/postlaunch-20260930/full-a1-preparation-h0-h2/`; profiling evidence is in
`TMP/postlaunch-20260930/digest-perf-tooling/`. The independent mathematical
review was by `final_requirements_audit`; the independent source/lifecycle and
cost assessment was by `runtime_order_pilots`. The latter implemented the
isolated patch; the former independently reviewed the code and tests. Native
correctness checks have passed; optimized performance validation is pending. Root coordinates
integration and final verification, without modifying production.
