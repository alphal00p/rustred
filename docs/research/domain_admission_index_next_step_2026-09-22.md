# Conditional next step: aggregate-filtered domain admission

Date: 2026-09-22. This is a read-only design recommendation, not an implemented
index or a measured speedup. Apply it only if the support-aware routing pilot
still identifies queue admission as the bottleneck. The semantic-only stopped
pilot performed 8,157,472,074 containment checks, including 1,407,523,725 reverse
maintenance checks; that establishes motivation, not expected acceleration.

**Subsequent implementation update:** the support pilot confirmed the
admission bottleneck. This proposal is now implemented;
36 focused tests and independent source review pass. Saved-descriptor replays
measure index-only improvements, while the full application/Python gate and
matched production campaign remain pending. See the
[implementation and timing audit](domain_admission_index_audit_2026-09-22.md).
The design below records the original recommendation, not an additional claim
of complete five-loop coverage.

## Smallest proposed index

Preserve the existing `(phase, owner)` partition, exact-key lookup, dominant
orthant fast path, cached native `DomainPowerSummary`, and all queued obligations.
In the unlimited-comparison lane, group retained maximal candidates by the exact
summary signature `(A_max, R_max, D_min)`. Each signature group holds ascending
admission IDs. These are topology-independent domain features, not benchmark
names or raw input labels.

Start with a compact vector of signature groups and a hash index for insertion,
both using fallible reservation. This keeps storage linear in admitted domains
and avoids a pointer-heavy full feature trie. Lookup scans group signatures,
not every candidate, then visits IDs only in admissible groups. Empty group
slots may be reused or compacted without changing logical results.

For incoming nonempty Q, containing C must satisfy:

```text
A_max(C) >= A_max(Q)
R_max(C) >= R_max(Q)
D_min(C) <= D_min(Q).
```

Reject an entire group if any inequality fails. Otherwise call the existing
exact `contains` predicate on its IDs. Find the first valid ID in each eligible
group and return the smallest valid ID globally. A running best ID permits
skipping later IDs without a query-sized allocation or a heap. Keep the exact
and orthant fast paths in their existing priority order: the minimum-ID rule
applies to the same retained-candidate fallback they currently precede.

For reverse retirement by a newly admitted Q, reverse the inequalities:
`A_max(C) <= A_max(Q)`, `R_max(C) <= R_max(Q)`, `D_min(C) >= D_min(Q)`.
Inspect and mutate only eligible groups. Do not retain the current full-owner
`ids.retain(...)` scan on every insertion, or the serial maintenance bottleneck
will partly survive despite fewer exact checks. Group storage should become the
retained-candidate source of truth; diagnostics can flatten and sort when needed.

## Why it cannot miss a valid container

For sets C and Q, Q contained in C implies that the maximum of every bounded
linear form over Q is no greater than its maximum over C, and the minimum is
no smaller. The three selected forms are necessary components of the exact
native summary predicate. Thus the filter cannot remove a true container.
Passing it is not a containment proof: the unchanged native predicate still
checks all coordinate and aggregate extrema. Reverse filtering uses the same
argument with C and Q exchanged.

This preserves the existing maximal-candidate set and deterministic fallback
choice. Index retirement removes no raw key, domain, FIFO job, or pending work.
A pending container remains an obligation, never evidence of completed closure.

Special cases must remain explicit:

- Store upper infinity and lower negative infinity as ordered variants, not
  finite sentinels or unchecked sign negations. Preserve the native u128/i128
  aggregate ranges.
- Empty summaries have no extrema. Handle them separately: an empty query is
  contained by every candidate in its existing phase/owner partition; an empty
  container cannot absorb a nonempty query. A nonempty new container can retire
  empty index entries without retiring their queued obligations.
- Reserve new group/index/ID storage and preflight accounting before retirement
  or publication. On failure, existing containing representatives must remain
  intact. A configured index-memory fallback may use the exact linear scan;
  it must never discard candidates.
- Leave the explicitly finite-comparison-cap lane unchanged. Count filter work
  separately from authoritative full comparisons, and retain checked counters.

## Cheap receipt inspection

A streaming inspection of
`TMP/correlated-routing-pilots.jw6Bwo/shared-owner-campaign.r8txq1a3/result.json`
read 125,504 committed records, including the final partial record. It found
209 phase/owner groups and 7,208 distinct **raw emitted** `(A cap, rank cap,
D lower)` signatures, or 17.41 records per signature on average. The largest
phase/owner group had 41,989 records and 327 raw signatures; the maximum raw
signature count for any phase/owner was 378.

This took 2.45 s wall, 2.34 s user CPU and 0.11 s system CPU, with 12,348 KiB
peak RSS. It was read-only, used one CPU, and launched no solver or build.

These are NOT exact-summary signatures, current maximal-candidate membership,
or the complete stream of accepted and rejected admission requests. No native
geometry was reimplemented to obtain them. The result suggests repeated feature
groups are worth measuring, but it neither gives the actual proposed index size
nor predicts filter selectivity or speedup.

## Gate before adoption

Replay the same complete bounded admission stream, including rejected requests,
through the current exact semantic baseline and proposed index. Require identical
returned IDs, new admissions, retained candidate sets and pending obligations.
Retain exhaustive small-domain inclusion checks, and add mixed infinities,
empties, phases, retirement chains, ties, allocation/counter failures, and
parallel-publication parity. Existing completed-descriptor replay is only a
preliminary diagnostic; it omits rejected requests and some original histories.

Measure signature groups visited/rejected, IDs visited, exact forward/reverse
checks, maintenance and summary-build time, coordinator CPU, index bytes/RSS,
and total admission time. If signatures are almost unique or rejection is weak,
the extra grouping can lose rather than win. Require bounded growth and a
repeatable measured benefit before making it the production policy.

A full `2N+6`-feature trie can prune more strongly but has greater node count,
allocation, locality and deletion complexity. Consider it only after the small
index's selectivity is measured and found inadequate; no general polyhedral
solver or new CAS operation is required for either design.
