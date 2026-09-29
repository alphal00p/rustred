# Registered follow-on optimization candidates

Date: 2026-09-29. These are proposals, not measured speedups or deployment
recommendations. G2′/rescue integration and its correctness gates take priority.
The governing scope and operational limits are in `CODEX_PROGRESS_PLAN.md`.

## 1. Lean telemetry: next narrow coordinator slice

Owner: `bounded_helpers_bmw`; independent critique pending implementation.

Mechanism: capture typed scalar pool statistics under the mutex, serialize
after releasing it, and update/reuse the lean JSON objects rather than
rebuilding their maps and constant strings after every commit. Preserve fresh
attempt counters before every existing checkpoint callback. Simply throttling
`set_parallel_lean` is unsafe: checkpoints persist that state, and a Ready test
uses `completed_slots_reclaimed` in a callback to release a waiting inspector.

Evidence: historical profiles attribute 2.5–5% of coordinator samples to this
path. This predates combined LC2/G2 and is not a predicted whole-walk gain.
Current source is `walking/parallel.rs::snapshot_tier` and
`walking/execution.rs::set_parallel_lean` in `rustred-app`.

Smallest test: compare every snapshot across synthetic commits, resume totals,
overflow, failures and reclaimed slots. Follow with Ready multi-prefix replay,
checkpoint/restore and Ordered exact-result controls. Preserve lean/detailed
key sets and do not retain stale detailed fields or decode failure IDs twice.

Falsifier: any changed non-timing counter or checkpoint semantics; absent
allocation/lock-cost reduction; or no useful matched campaign improvement.

## 2. Helper preparation: preserve the negative result

Preparation's historical 32% coordinator-wall share includes useful containment
work, stragglers and waiting; it is not all removable scheduling cost. Changing
the fixed 256-record batch boundary changes reuse/containment behavior.

The previous minimum-eight grain experiment remains rejected: median paired
ratio 0.99885, preparation 15.48% slower. Source:
`five_loop_admission_grain_2026-09-25.md` in this directory.

Only a narrow allocation cleanup is presently identified: order-preserving,
fallible in-place filtering of the replay suffix, eliminating its extra vector.
Test mixed replay/new/non-admission batches with a mid-batch error, preserving
all replay calls, first-error behavior, cancellation and queue snapshots.
Expected benefit is small and unmeasured. Do not construct another speculative
pool or snapshot architecture without new evidence; that overlaps epoch work.

## 3. N2: redundant geometry copies

Owner: independent `checkpoint_final_audit` read-only registration.

Target the scratch and copied vectors in `owners/domains/applied/geometry.rs`,
using the existing checked `LatticeBox::try_from_preallocated` ownership path
where appropriate. Retain callbacks, iteration order, correlated A/R/D bounds,
unbounded endpoints, checked overflow and logical resource charges. Scratch
must not be mutated while borrowed by a callback. Keep algebra in Symbolica.

Do not rewrite `power_domain::project`: its current implementation already uses
fixed arrays. The historical 10.39% projection CPU share is not an estimate of
removable allocation overhead. Earlier duplicate-projection removal changed
median traversal by only -1.46% and -0.059%, with overlapping ranges; see
`finite_closure_native_profile_2026-09-23.md`.

Smallest test: existing applied-geometry differential tests, including fixed
and absent crossings, correlation, bounds above u32, cancellation, budgets and
overflow prefixes. Then identical event/counter and successor/frontier/problem
records on C-4L/C-5F with G2 enabled in both arms. Park if no useful whole-walk
benefit; G2 may remove much of the native work this would optimize.

## 4. Research: exact pending D-band coalescing

Owner: `work_reduction_research`; separate critique by `checkpoint_final_audit`.
Status: **parked after negative opportunity census**, no engine implementation.

Within a bounded epoch-local miss batch, group domains with exactly equal
program/rescue view, phase, owner, coordinate bounds, numerator-rank bound and
positive-power bound. Only the integer D interval may differ. Overlapping or
adjacent D intervals can then be merged without adding points. For a common
remaining predicate B, `B ∩ [7,9]` union `B ∩ [10,12]` is exactly `B ∩ [7,12]`.

This is inspired by exact symbolic-state merging, not by a claim that timed
automata theorems establish IBP closure. The primary paper explores convex
zone merging and the importance of merge policy/overhead:
[André et al., Efficient Convex Zone Merging in Parametric Timed Automata](https://arxiv.org/abs/2212.04802).

Unlike previously rejected hull widening/dense cells, this adds no input points.
Unlike general admission-time union lookup, it proposes signature grouping and
interval sorting within a bounded batch, not a scan over historical domains.
Unlike G2′, it combines still-uninspected obligations rather than borrowing an
earlier local inspection. No reserved job, initial query, accepted stream,
quarantined node or published history may be rewritten. A union remains a
pending obligation; all original requests need exact dependency accounting.
All already-admitted domain images/IDs, including Pending ones, remain
immutable. Start with unassigned P2 misses; a later extension could append a
new union and transfer genuinely Pending/unprotected originals only through
normal verified-containment aliases. Exclude dead/quarantined work entirely.
The synthesized union needs its own digest/native summary, deterministic first
request position and checked tokens for every original request. It cannot
become a G2 anchor before ordinary native publication. The current invariant
that a survivor uses one original image would need an explicit, audited
geometry-construction extension, not a bypass.

First falsifier: inspect immutable finite-five-loop and r1a12 receipts for
matching signatures in windows of 16/64/256 records, but first establish what
the records actually retain. Publication windows without batch membership are
only descriptive geometry proxies, not actual epoch miss batches or rigorous
opportunity bounds. Record that limitation and use post-G2 data when available.
A weak opportunity count can justify deferral, not disprove opportunity in the
unrecorded P2 stream. If receipts lack the necessary information, defer rather
than infer a reduction from a whole-run histogram.

Only after that: test exact union-input equivalence and compare native output
partitions/downstream work. An exact input union can still lead to more costly
guard partitioning. Count additional union IDs and edges. Preserve interval
overflow/infinity semantics, full-signature checks, replay and quarantine.
Test a feasible common base with D=0 and D=2 requests: never fill the D=1
hole. Separately test exact adjacent union and rejection of Reserved transfer.
Complexity estimate: O(m N) full-signature work plus O(m log m) interval sorting
for a batch of m, dimension N; O(m) auxiliary references (or O(m N) if full
signatures are copied). Durable provenance integration is a separate task.

### Census result (2026-09-29)

The legacy G2-on receipts lack actual miss/admission batches, so no misleading
publication-window histogram was substituted. Instead, an independent source
audit supported reconstruction of historical **G2-off epoch C-5F post-antichain
survivor birth cohorts** from paired record/edge streams. All 1,215,537
records/domains and 12,314,557 edges passed attribution and ordering checks.
Across 48,412 birth merges, the 1,215,536 noninitial survivors had **zero
identical base-signature pairs and zero possible coalescences** in each of the
tested within-cohort windows (16, 64, 256).

Analysis cost: 26.304 s, peak RSS 201,396,224 bytes. Evidence (including source
hashes and exact argv):
`TMP/codex-integration/coalescing-census.Gs3IcM/result-r2.json`.
The first attempt failed closed because the parser omitted the existing
partial-initial-inspection record kind; its receipt is retained as `result.json`.
Source inspection justified that one correction; birth/edge checks were not
weakened. Startup tests covered holes, adjacency, overlap, infinity and unequal
base signatures.

This is negative evidence for this narrow candidate on this workload, not a
theorem about all pending domains and not post-G2 evidence. Reopen only if an
actual post-G2 miss-cohort trace shows useful exact coalescing opportunity.

## 5. Deferred: backward closed-source covers

For an exact rule with targets T_j(n), recursively closed target covers C_j
would give a sufficient source region `G ∩ ⋂_j T_j⁻¹(C_j)`, with G retaining
applicability, original poles and descent. Every RHS obligation is required.
This differs from N4's avoidance of coefficient work on existing forward
images; it would derive a reusable source region before later fragments arrive.

Do not treat merely locally discharged G2 anchors as recursively closed here.
The current guarded pullback engine rejects correlated A/D bounds, directly
relevant to production. Exact preimage geometry, provenance and replay therefore
make this nontrivial. Reopen only if a small sign-stable, shift-only case from a
completed control demonstrably covers useful later work beyond G2. No general
SMT/Presburger service is proposed. No implementation is authorized by this
registration alone.

None of these candidates proves termination from bounded input rank, zero
frontiers, a smaller queue, or higher CPU utilization.
