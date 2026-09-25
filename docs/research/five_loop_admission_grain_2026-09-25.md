# Admission task granularity: controlled experiment

This experiment tests a narrow scheduling hypothesis from the live five-loop
profiles: whether coarser Rayon lookup tasks reduce overhead in immutable
successor-admission batches. It does not change production scheduling or prove
recursive closure. The [live profiles](five_loop_slow_inspection_parallelism_2026-09-24.md)
also show inspection-heavy intervals where this cannot remove ordered-head
blocking.

## Implementation boundary

The existing admission engine has a private `cfg(test)` constructor accepting a
nonzero minimum indexed task length. The candidate uses eight records, compared
with the existing constructor and iterator. Production has neither the new
field nor a new runtime branch, public option or schema.

Both variants keep the same immutable 256-record batches, lookup eligibility,
24-helper pool, cancellation, replay filtering, prepared-token validation and
original-order exclusive commit. Rayon minimum length is a splitting hint,
not a guarantee of eight costly lookups per task or exactly 32 tasks per batch.

Ordinary test builds count index visits with shared atomics that do not exist
in production. Both timed variants disable those counters before work. A
test-only queue preference propagates the setting to existing **and newly
created** owner buckets. Defaults and deserialization keep instrumentation
enabled for normal diagnostic tests. Nothing resets observed counts, and the
benchmark checks that all counters stay disabled and zero. The remaining test
layout/branch overhead is shared by both variants, not claimed to be absent.

## Frozen workload and timing scope

The two retained Ready checkpoint images contain 298,219 and 358,373 logical
domains. Their complete common prefix is identical. The benchmark restores
the earlier queue, index and responsibility ledger for each variant, then
submits all 60,154 appended descriptors in original ID order as synthetic
admissions. The final batch contains 250 records.

This is **not** historical execution replay: intervening publications and
refills, rejected/reused proposals, original callback flags and job-local
reuse are absent. Neither variant is required to reproduce the later
checkpoint's lifecycle. The scratch result is never published as a resumable
campaign checkpoint. A live 20-second sample had 359,247 successors but only
13 new admissions; this insertion-focused workload cannot estimate that
reuse-heavy stream's total speedup.

The retained suffix introduces 387 new phase/owner keys in 2,814 descriptors.
An initial preparation caught the incorrect assumption that disabling counters
on existing buckets sufficed. It failed before any timing. The corrected
preparation preserves every descriptor, with no pre-created semantic buckets,
and explicitly propagates the test preference to future indexes.

Four fresh process pairs are fixed before execution: default/min8, min8/default,
default/min8, min8/default. Each uses 24 helpers plus one coordinator on physical
CPUs 64–88, independently of the live campaign's CPUs 0–49. Protection is RAM
only: 100 GB hard, 95 GB cooperative, and 20 GB host reserve; there is no elapsed
or mathematical-work limit. Shared-host activity remains a caveat.

Admission-loop wall time includes the same per-batch receipt callbacks.
Preparation and ordered-commit spans are recorded separately. Restoring and
hashing the queue, constructing input events, pool teardown and final
canonicalization are outside that loop. Whole-process CPU/RSS includes setup
and checking, so it is not an admission-kernel allocation measurement.

Within and across pairs, require identical complete canonical queue/index/
ledger images, per-batch counters, accepted events and non-timing admission
metrics. Only the unordered outer owner-bucket map is sorted for comparison;
domain IDs and inner index/ledger order remain evidence.

## Correctness gates

Independent source review passes. The final optimized build passes 715
application/CLI/API/doctests, with zero failures and three ignored diagnostics
(one pre-existing, two explicit experiment tools). Focused admission tests
pass 17 cases; counter-switch tests pass two. The tests cover batch boundaries,
mixed callbacks, native-summary errors/overflow fallback, cancellations,
original-order outcomes, compacted replay prefixes, exact state comparison,
and preservation of index behavior with disabled instrumentation.

All six experiment-owned Rust files pass focused formatting checks. The
repository-wide check still reports seven unrelated pre-existing import/module
ordering differences; those files are not changed by this experiment.

Both timed variants use the same optimized test executable, SHA256
`1c2ebd2443142e8522bae0a796fb19a6886784c48ceb16683dbb49253828c7e0`.
Its profile is release opt-level 3, LTO off, 256 codegen units, incremental on,
debug off. It is suitable for this same-executable paired comparison, **not**
an absolute comparison with normal production-release timings. Build and test
time are excluded from benchmark results.

Raw gates are retained in `TMP/admission-grain-gate.lV0U5q/`; the failed initial
preparation is in `TMP/admission-grain-replay.OC2sWQ/`; the corrected fixed
matrix is in `TMP/admission-grain-replay-v2.3qRfZR/`. Runtime results remain
separate from the correctness-gate timings above.

## Completed measurements

All four fixed pairs finish successfully. All eight exact semantic receipts
match: the complete canonical queue/index/ledger digest, 235 batch receipts,
60,154 accepted synthetic events and every non-timing admission metric.
Both modes perform 16,068,079 speculative containment comparisons and admit
all 60,154 descriptors without deduplication. The repeated effects include
19,567 candidate retirements and 18,982 responsibility transfers; there are
no lifecycle publications in this scratch replay.

Times below are admission-loop wall seconds, excluding restore/verification.
Positive change means that the minimum-eight variant is slower.

| Pair | Execution order | Default | Minimum eight | Change |
|---|---|---:|---:|---:|
| 1 | Default, minimum eight | 1.204559 | 1.220487 | +1.32% |
| 2 | Minimum eight, default | 1.178524 | 1.250483 | +6.11% |
| 3 | Default, minimum eight | 1.234027 | 1.191781 | -3.42% |
| 4 | Minimum eight, default | 1.369724 | 1.348474 | -1.55% |

The median paired min8/default ratio is 0.99885, effectively flat in this
small noisy sample. Summed loop time is 4.98683/5.01123 s (+0.49%). The
separately computed variant medians are 1.21929/1.23549 s (+1.33%); that is
not the median paired ratio. None of these summaries establishes a useful
wall-time win.

Preparation is slower with minimum eight in **all four pairs**: its separate
median increases from 0.12714 to 0.14682 s (+15.48%). Ordered-commit medians
are 1.08811/1.08465 s, nearly unchanged. Commit spans account for approximately
87–90% of the measured loop. Process CPU medians during that loop fall from
3.285 to 3.180 s; lower CPU use alone does not meet the wall-speedup objective.
These phase measurements are elapsed spans, not an exclusive CPU profile.

Whole processes take 22–24 s, largely setup and verification, with GNU-time
peak RSS of 2,248,980–2,265,100 KiB. No owner programs or CAS workloads are
loaded. All runs use the intended 25-core affinity and finish without resource
stops, forced termination or supervisor errors. The matrix spans
05:17:35–05:19:08 UTC; all before/after live brackets retain checkpoint 9 with
no active write.

The independent raw-receipt audit passes for all eight variants, including
exact batch/state comparison, instrumentation state, resource boundaries and
the distinction between paired ratios and ratios of variant medians.

Keep the minimum-eight option test-only and retain the production iterator.
The experiment does not support granularity tuning as the fix for five-loop
utilization. Continue the separate full-drain Ordered/Ready controls and
measure the actual reuse-heavy stream before changing admission policy.
