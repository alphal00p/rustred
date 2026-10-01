# Reusing stale snapshot-negative lookup prefixes

Status, 2026-10-01: independently reviewed design; isolated implementation
and tests being prepared. No measured speedup or production change.

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
| Native traversal wall time | 1,202.645 s |

Source resolution is 18.42% of traversal, but includes query construction,
hashes, and positive verification as well as lookup. This is not a lookup-only
timer or an achievable speedup. The rows above do not measure snapshot lag.

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

Unbound, reference, synthetic AllMiss and raw restored results use full lookup.
Resume reissues unfinished jobs with new session/sequence/version/watermark
bindings; only freshly dispatched jobs may acquire the new eligibility.
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

The ongoing frozen old/new compact-row benchmark must finish independently;
its results cannot validate this later implementation.

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
cost assessment was by `runtime_order_pilots`. The isolated implementation is
assigned to the latter; its separate code audit is pending. Root coordinates
integration and final verification, without modifying production.
