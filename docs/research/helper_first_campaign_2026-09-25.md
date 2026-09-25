# Helper-first input admission: measured control and future five-loop preset

## Decision

Prepare a separate future five-loop input snapshot with the existing broader
helpers first within each owner, keeping first-seen owner order. Do not change
the live campaign. Keep shared Ordered/H256 execution, joint pruning off and
independent starting-owner execution off. Preserve all 67 owner payloads and
134 explicit queries, including the existing positive-power/rank/difference
bounds. No helpers are added or removed; no IBPs are regenerated.

This is a modest evidence-backed input-order improvement, not a demonstrated
five-loop optimum. A completed four-loop FG control supports helper-first
admission; it does not justify additional wider helpers or a true bottom-up
sector scheduler. The [production recipe](../shared_owner_campaign_driver.md#recommended-fresh-attempt-existing-helpers-first)
prepares without launching and provides separate start/resume commands.

## Completed four-loop control

All seven native walks completed with zero final queued/pending domains,
frontiers or errors. An independent read-only audit checked the input queries,
their admitted representatives, descendant accounting, outputs and timings.
The scope is the FG family only, not the complete four-family four-loop census.

| Input order / extra helpers | Whole-command wall (s) | Traversal (s) | Native inspections | Explicit queries |
|---|---:|---:|---:|---:|
| Original order | 18.526 | 12.951 | 98,869 | 248 |
| Existing helpers first | 16.439 | 11.594 | 98,627 | 248 |
| Helpers first + ascending support | 16.243 | 11.563 | 98,625 | 248 |
| Add R13 helpers + support order | 16.831 | 11.661 | 100,094 | 372 |
| Add R13 and R14 + support order | 19.699 | 13.172 | 110,084 | 496 |
| Original order, repeat | 18.633 | 13.260 | 98,869 | 248 |
| Helpers first + support, repeat | 16.305 | 11.584 | 98,625 | 248 |

Conditions: same optimized native executable, all 124 FG saved owners/routes,
six compute workers pinned to CPUs 64–69, shared queue, Ordered publication,
pruning off, 100 GB RSS guard, 20 GB host reserve, no time/work cap and no
overlapping native cases. Whole-command time includes native preparation,
import, traversal, saving and output; it excludes compilation, input preparation
and post-run Python audits. Peak sampled aggregate RSS was about 1.15 GB for
the unexpanded cases and 1.26 GB with both additional ranks.

Original requests retain A<=19, R<=12, D=A−R>=7 and their coordinate bounds.
Existing helpers have R<=12 with no A/D bound. The expanded controls add the
same owner's helpers at R13 and R14, not free hints: these are additional
obligations whose descendants reached rank 15/16 rather than 14.

Helpers-first alone reduced whole-command time by approximately 11.3% in its
single comparison. The support-ordered configuration reproduced approximately
12% reductions in two sequential pairs, but helpers-first alone was not repeated
and the evidence does not isolate a support-sorting benefit. Consequently the
production change keeps existing owner order instead of adding support sorting.
These local samples are not a statistically controlled cross-family prediction.

## Why ordering helps, and what it does not do

The native walker admits all input queries before traversal. An already-admitted
broad same-owner helper can absorb a later narrower request under the existing
exact containment checks. A narrower request admitted first is pinned and is
not subsequently removed just because a broader initial helper appears.
Helpers-first reduced distinct initial native admissions from 248 to 124 while
retaining all 248 explicit query IDs. Native Apply-operation counts fell from
9,280,817 to 7,975,678, whereas native inspections fell by only 242.

The complete initial helper index is available before traversal in either
order. This change is not a scheduler that solves every lower sector before
larger sectors. Descendant ordering, rule identities, guards and exact
application machinery are unchanged. Increasing helper rank also increases
what the walk must discharge; the expanded controls show why adding more
helpers cannot be assumed to improve performance.

The first historical audit assumed one initial native admission per explicit
query and falsely rejected the reordered runs. The corrected local audit kept
the original descendant/status/accounting gates and checked every query ID
against its same-owner containing representative. No native result was changed
or rerun to obtain an audit pass. Independent examination confirmed this
many-to-one mapping, empty final worklists and reproducible integer counters.

## Preparation interface and boundaries

Generic staging exposes `--query-order preserve|helpers-first` (default
`preserve`). `production_saved_owner_campaign.py --prepare-from SOURCE` opts
into helpers-first for a new destination; an explicit `preserve` provides a
control. It uses the existing `owner-anchor-` ID convention solely to prioritize
staging-generated helpers. Within an owner, higher-rank helpers precede
lower-rank helpers, with stable ties; the native engine still verifies every
inclusion. Nothing depends on a loop count or a topology name.

Fresh preparation refuses an existing destination, a source/destination nesting,
or combination with resume. The copied query objects and program bytes are
unchanged; original query bytes and an ordering receipt are retained. An
existing campaign's binary, frozen policy, input bytes and checkpoints are
never reordered. Only explicit `--start` launches the solver. New preparation
does not reuse old traversal progress; changing initial admission order is a
fresh run, not a checkpoint migration.

## Implemented preparation check

All 103 Python example tests pass, including generic helper ordering, unchanged
query objects and source bytes, rejection of reused/nested/symlink-overlapping
destinations, explicit-launch behavior, frozen resume policy and the existing
700 GB RAM-policy tests. No Rust solver change or rebuild was required.
An independent implementation/documentation audit passed and separately checked
the actual prepared query identities, helper order, policy and absence of a run.

The separate local snapshot `campaigns/five-loop-saved-helpers-first` is prepared
but **not launched**. Its 67 copied payloads total 1,280,854,595 bytes and match
the source payload hashes. All 134 query objects match by ID; first-seen owner
order is retained, with each helper now before its original request. All 67
helpers retain R<=15; 54 retain A<=24 and 13 retain unbounded A. The new query
SHA256 is `fa8bda0851c7116c50e05e3895c6614bcd9dd02dcae4578631b83d73a8b33f89`.
The source inputs, frozen executable/policy receipts and active-run record have
unchanged hashes, sizes and modification times after preparation. No new run,
active-run record or checkpoint exists in the prepared destination.

The frozen fresh policy uses the release executable below, 50 workers on CPUs
0–49, shared Ordered/H256, a 500 GB requested RAM guard with 5% margin, hourly
checkpoints and uncapped cumulative work. It is independent of the old run's
frozen executable/checkpoint. This check establishes safe preparation, not
five-loop traversal completion or a measured five-loop speedup.

Local detailed evidence is retained under
`TMP/four-loop-helper-order.Iy9nQt/`, including `RESULTS.md`,
`INDEPENDENT_AUDIT.md`, immutable inputs, raw results and closed measurements.
Executable SHA256:
`030e09ef661cc69b5acc0501376b8174a518eb95cb0bf0f656835e84613f6c60`.
The benchmark evidence is untracked; this document records its conclusions
without adding saved payloads or reference material to the repository.
