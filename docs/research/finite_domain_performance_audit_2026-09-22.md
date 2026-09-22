# Five-loop finite-domain performance audit — 22 September 2026

**Post-audit gate update:** `app-corrected.log` in
`TMP/semantic-containment-gate.qn9MTE/` now passes 361 application tests with
one ignored diagnostic, including both complete parity loops. The count fixes
assert the equivalent requests share a domain ID; parity assertions were not
removed. The 82 integration tests pass separately. References below to two
failing counts describe the initial audit snapshot, not this corrected gate.

## Executive assessment

The current blocker in the measured bounded-domain pilots is **excessive
conservative dependency work and serial admission/containment**, not exhausted
RAM, a demonstrated missing IBP, or a proven inability to reduce the selected
inputs. Fifty workers were configured throughout; some phases used them well,
but the constrained pilot eventually averaged only about two busy cores while
its coordinator admitted completed worker results. Increasing the worker count
or the completed-result buffer alone does not address that bottleneck.

Two new diagnostics provide actionable evidence:

1. Semantic domain containment reduces index-replay comparisons almost fivefold
   and index time about 4.27-fold on 49,686 saved completed descriptors. This is
   **not** a measured campaign speedup. The integrated application gate currently
   has two failing expected-count assertions and must be resolved first.
2. Native affine-row support bounds strictly tighten 17,304 of the 27,807
   observed mapped root covers after existing native projection. Full-family
   permutation routing would help **none** of these observed map calls. The
   support-aware alternative is therefore the more relevant next routing slice.

No complete finite renormalizable five-loop envelope has closed. No new IBPs,
terminal evaluations, or five-loop numerical Vakint results were produced by
this audit. Its helpers perform native map verification, domain arithmetic and
saved-descriptor replay, not source search or coefficient back-substitution.

## Completed controls versus incomplete campaigns

Times below separate native traversal from preparation. RSS and CPU in the
campaign rows are supervisor samples, not exact transient process peaks.
All runs use saved owners/rules; these are **not IBP-generation timings**.

| Workload | State / stop reason | Traversal | Preparation or other time boundary | Complete / pending domains or nodes | Peak sampled RSS |
|---|---|---:|---|---:|---:|
| One concrete starting key in each of 67 owners | Completed for these 67 keys | 4.265 s | 108.233 s application; 112.049 s supervisor | 348,652 operational nodes / 0 | 5.645 GB |
| 50 concrete diagonal stress roots | Completed for these 50 keys | 1,552.875 s | 1,666.006 s application; 1,695.856 s supervisor | 54,695,087 operational nodes / 0 | 26.109 GB |
| Unconstrained diagonal, maximal index | Stopped at 50-million committed-event allowance | 241.852 s | 105.182 s preparation | 137,684 / 127,080 | 11.054 GB |
| All-67-owner binary-dot/R0 control | Stopped at 500,000 scheduled-domain allowance | 81.587 s | 104.997 s preparation | 90,428 / 409,571 | 8.404 GB |
| Correlated diagonal A≤24, D≥10, R≤10 | Cooperatively stopped for measured admission pressure | 106.956 s | 102.701 s preparation | 49,686 / 145,431 | 9.589 GB |

Each stopped domain campaign also has one partial record. Their observed
unresolved-frontier count is zero, which does **not** discharge the pending
queue. Stops in the first two domain controls were explicit work allowances,
not elapsed deadlines. The correlated pilot stopped before either diagnostic
allowance or the 450/500 GB soft/hard RSS limits was reached. Its supervisor
record reports `hard_stopped=false` and an operator cooperative stop.

The 67-key and 50-key concrete controls establish complete dependency traversal
for their specific finite inputs, not every integral in those owner classes.
They perform no coefficient back-substitution. The local correlated diagonal
matcher separately covers all 980 admitted points in 28 selected-rule regions,
with zero partition or saved-singleton-oracle mismatches; that 0.008475-second
local match is not recursive closure.

The unconstrained diagonal contains 2,560 starting points; the correlated
diagonal contains 980 and retains all 50 stress roots. The all-owner binary-dot
control contains 58,400 starting points. These are different workloads: their
final-row timings cannot be compared as before/after speedups. None covers the
entire marginal R14 or full-jet R15 physical envelope described in
[the finite-domain plan](../finite_starting_domains.md).

## Workers, queue growth and bottleneck attribution

All domain campaigns requested 50 workers, affinity CPUs 0–49 and single-thread
nested compute pools. The measured utilization depends strongly on the phase:

| Measurement window | Effective CPU observation | Interpretation |
|---|---|---|
| Maximal-index unconstrained diagonal, elapsed 110–330 s | Mean 43.11 busy cores; range 35.27–47.59 | Useful parallel worker activity in this phase |
| All-owner R0 control near elapsed 178 s | 1.084 busy cores; all workers idle in nearby snapshot | Serial admission dominated after result escrow filled |
| Correlated D10 pilot, elapsed 125–200 s, 37 samples | Mean 2.084 busy cores; range 1.249–3.377 | Persistent coordinator bottleneck despite 50 configured workers |
| Completed 50-root concrete control | 31,294 sampled CPU seconds / 1,695.856 supervisor seconds ≈18.45 average cores | Includes setup; not a claim of continuously busy workers |

The correlated pilot completed 49,686 domains but scheduled 195,118. It made
4,971,600,936 containment comparisons, **including** 987,619,360 reverse-index
maintenance comparisons. Do not add the maintenance subset a second time.
Its completed-result escrow reached the 65,536-entry limit while accounting
for only 2.561 GB, below its 8.590-GB byte allowance. Thread snapshots showed
the coordinator running and most workers waiting in futex calls. Progress
continued, so this was not a deadlock; the pending queue nevertheless grew.

This evidence identifies serial admission as a bottleneck, but is not a
stack-sampled percentage breakdown of every function. It also does not provide
a defensible end-to-end completion ETA. No draining phase has been observed
for these broader domain workloads. Their sampled memory is far below 500 GB;
blindly allocating more RAM would not prove convergence within fifteen hours.

One earlier optimization already has a valid matched-prefix observation:
74,326 completed records agree byte-for-byte after removing only their timing
field between the historical-list and maximal-index diagonal runs. Traversal
to that prefix improved from 315.671–316.676 s to 108.625–109.632 s, or
2.88–2.92×. That is a single shared-host **prefix** result, not full closure.

## New semantic index replay

The new generic semantic summaries retain exact coordinate/A/R/D predicates
and compare their native extrema. An ignored read-only replay feeds the saved
completed descriptors into both the historical raw maximal index and the new
semantic maximal index. Six runs alternate raw-first/semantic-first, three of
each. All six reproduce identical admission/comparison counters and pass
post-run semantic containment checks.

| 49,686-descriptor replay | Raw index | Semantic index |
|---|---:|---:|
| Admitted descriptors | 49,686 | 25,207 |
| Live lookup candidates | 21,711 | 11,597 |
| Comparisons, including maintenance | 112,362,704 | 22,484,047 |
| Included maintenance comparisons | 56,181,352 | 9,182,246 |
| Median index wall time | 1.319154 s | 0.308513 s |
| Additional semantic reuse hits | — | 24,479 |

The median **paired** wall ratio is 4.2711×, with observed range
4.2314–4.3253×. The paired thread-CPU ratio is 4.2696×; comparison count drops
4.9974-fold. This agreement limits concern that the measured gain is merely
host scheduling noise, but six same-host diagnostics are not a production
confidence interval. The smaller 10,000-record replay shows almost no timing
gain (0.03838 versus 0.03748 s): summary construction has a real cost, and
benefit appears as duplicate/containment work accumulates.

The timed boundary includes owned descriptor clones, summary construction and
index admission. It excludes JSON parsing, type decoding and post-run
verification. Thread CPU comes from Linux's per-thread scheduler runtime, not
process-wide CPU. RSS was not recorded separately for this microbenchmark.
The input is only the **completed descriptor list**: it omits most proposed,
rejected, pending and speculative admissions from the original campaign.
Thus 4.27× must not be multiplied into the campaign time or a closure ETA.

At this audit snapshot, the new release core gate passes 2,758 tests with 32
existing diagnostics ignored; the isolated actual-source queue harness passes
27 tests with one ignored replay. The integrated application run ends with
359 passed, two failed and one ignored. All 82 separately invoked integration
tests pass. The two failures are fixed-count assertions in the routed-walk and
initial-orthant fixtures (actual counts three versus four and four versus five),
**before** their serial/parallel comparison loops. Thus these receipts do not
show a worker-parity mismatch; those test bodies have not reached that check.
Their cause and corrected unit-gate validation are pending; this report does
not assume they are harmless. The
previous correlated-domain release gate remains the last fully passing
application/integration baseline. No new campaign was launched during this
audit. The build, replay and support probes have finished, and no root-owned
build or solver remains running at the final snapshot. Build/test status is a
snapshot, not evidence of production closure.

## Routing census: reject the wrong optimization, measure the useful one

The native map census verifies every one of the saved 8,246 routing records.
It uses Symbolica exact rational matrices, native `symmetry::verify` and native
`permutation::compile`; there is no independently implemented CAS. The census
completes serially in 89.770 s internal / 89.80 s process wall, 88.90 s user CPU,
with 120,276 KiB peak RSS. Compilation is outside that diagnostic boundary.

Of 8,179 nonliteral maps, only 41 are full unit denominator permutations; the
other 67 accepted records are literal-owner identities. **None of the 32
nonliteral maps actually used by the constrained pilot is eligible.** Those
32 sources account for 27,807 mapped calls and 721,497 emitted conservative
Route children. Another 171 visited source masks / 8,495 records are native
known-zero exits, not unsuccessful map transports. All 8,138 rejected maps
have genuine nonmonomial rows, rather than failures of unrelated policy checks.
Consequently, a full-permutation fast path would not help this observed prefix.

A more relevant proposal uses the support of the already verified affine
numerator rows. For source numerator degrees `t_i`, target monomial degree
`beta_j` can only receive contributions from rows with `M_ij != 0`. A safe cap is

```
B_j = min(sum of relevant source upper degrees,
          source R maximum - sum of irrelevant source lower degrees).
```

For a positive source index with local lower `l_i` mapped to target `j`, retain
target local lower `max(0,l_i-B_j)` on surviving axes; pinching is impossible
if `B_j < l_i+1`. An inactive source row fixed to degree zero contributes
nothing. Constants and exact cancellations cannot invalidate these bounds.
This does not preserve the unsafe affine upper D bound and does not reuse
root-only inferred bounds in pinched children.

The observed-only native diagnostic verifies/prepares those 32 maps once and
analyzes all 27,807 saved mapped source domains using public native
`DomainPowerSummary`. It completes in 1.288465 s internal / 1.36 s process wall,
1.21 s user + 0.13 s system CPU, with 192,032 KiB peak RSS, exit zero. Both
diagnostics use single-CPU affinity. No numerator expansion or IBP solve occurs.

| Geometric effect in observed source domains | Records |
|---|---:|
| Uniform total-R bound strictly tightens root cover after native projection | 3,696 / 27,807 (13.29%) |
| Affine support bound strictly tightens root cover after native projection | 17,304 / 27,807 (62.23%) |
| Support bound strictly improves on the uniform-R result | 14,025 |
| Support forbids at least one singleton pinch permitted by the old weighted-rank condition alone | 20,340 |

The last row counts 68,659 individual axis cases. It is **not** a count of newly
excluded complete successors: the existing later A/D projection may already
reject some of those singleton pinches. Likewise, the affected records emitted
634,217 old Route children, but that is only associated traffic, **not 634,217
children proved removable**. No pinch subsets were enumerated. The strict
root-cover improvements, by contrast, compare actual native projected sets.

The mapped-route local inspections themselves total only 2.779 worker-seconds
in the old receipt. The expected opportunity is therefore smaller and fewer
downstream domains, not merely making a cheap routing function faster. A new
end-to-end pilot must establish whether that opportunity translates into less
admission work and eventual queue drainage.

## Recommended next experiments, in order

1. **Resolve integrated gate failures, then replay the same correlated D10
   campaign with semantic admission.** Keep the input, saved owners, routing
   witnesses, 50-worker budget and limits unchanged. Measure preparation,
   traversal, comparison/retirement counts, queue growth and effective cores.
   Stronger reuse may change completed-descriptor ordering, so do not demand
   byte-identical history as a substitute for semantic correctness or compare
   unlike prefixes as if they were the same workload.
2. **Implement and independently audit support-aware mapped bounds.** Use the
   existing native verified row supports and domain service. Test concrete
   transport endpoints, zero-degree rows, affine constants, multiple pinches,
   unbounded caps and checked wide arithmetic. Start with lower bounds and
   impossible-axis pruning; do not begin with a new symbolic algebra engine.
   The measured 62% root-cover tightening makes this a better justified slice
   than full-permutation specialization.
3. **Measure this routing slice separately before combining claims.** Compare
   equivalent local covers/endpoints, then the same recursive input. Record
   actual masks and children eliminated, not only the number of potentially
   affected records. Keep source-derived bounds independent of target-root
   projection and do not clip descendant ranks to the starting R10 limit.
4. **Only then widen the physical input.** If the queue still grows, collect
   per-owner/phase admission profiles and reconstruct concrete witnesses for
   real missing-rule frontiers. The existing unfinished prefixes show no
   missing IBP; speculative source generation is not justified by queue growth
   alone. A larger dominance index or more cores should follow that evidence.

## Evidence locations and interpretation limits

All temporary paths are ignored local evidence; none contains a persisted
license. No production implementation was edited by this performance lane.

- Concrete controls: `TMP/finite-entry-gate.2GG13m/shared-owner-campaign.p3kv5i96/`
  and `shared-owner-campaign.k1ou4fev/` in the same directory.
- Older domain controls and matched-prefix audit:
  `TMP/bounded-routing-pilot.4kY19R/RESUMED_PERFORMANCE_AUDIT.md`.
- Correlated input, local matching and stopped traversal:
  `TMP/correlated-routing-pilots.jw6Bwo/LOCAL_MATCH_AUDIT.md`,
  `RECURSIVE_D10_AUDIT.md`, and `shared-owner-campaign.jtsg2q0t/`.
- Semantic replay/gates: `TMP/semantic-containment-gate.qn9MTE/`;
  six `replay-full-{raw-first,semantic-first}-{1,2,3}.log` receipts.
- Native census, support proof, helper inputs, source/binary hashes and timing:
  `TMP/route-permutation-census.mWBz2v/RESULTS_AUDIT.md`,
  `AFFINE_SUPPORT_BOUND_PROOF.md`, `support-stdout.jsonl`, `support-time.log`,
  and `semantic-replay-audit.json`.

These shared-host runs were not collected on an otherwise idle dedicated
machine. Their bounded-workload counters and reproducible native set checks
are stronger evidence than an extrapolated wall-time promise. There is still
no measured basis to promise full physical-envelope completion in fifteen
hours, and no new full-family closure claim.
