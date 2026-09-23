# Bounded reuse of completed inspector slots

## Scope

The preceding concurrent-owner controls allow several inspections within one
sector but remain slower than Ordered. They report zero full-buffer producer
backpressure. A different limitation is demonstrated: finished jobs retain
physical worker slots while waiting behind the owner's publication head.

This slice shares the existing bounded successful-result store between the two
schedulers. It changes neither loaded IBPs nor domains, guards, descendants,
terminal policy or mathematical authority. Ordered remains the default.

## Storage and responsibility

Only successful, non-running, fully flushed jobs may move out of a physical
slot. Their chunk and native summary move together into the shared store. They
remain unpublished responsibilities in the per-owner FIFO and pending registry.
Moving a job does not admit its events, charge native totals twice or discharge
its ledger entry. Failed jobs cannot take this path.

The store retains the existing limits of 65,536 entries and 8 GiB of accounted
storage. This includes result metadata, event payload and outer vector spare
capacity, but is not an RSS guarantee. Hash-table capacity, queue metadata,
allocator overhead, rules, native scratch and other buffers remain additional.
The outer process memory supervisor remains necessary. When a limit is reached
or optional allocation fails, ordinary slot-held execution remains available.

The owner coordinator separates physical occupancy from pending publication:

- Its active job list contains at most the native worker budget.
- Per-owner FIFO fronts identify publishers without scanning retained history.
- It polls at most one chunk per owner and at most the worker budget per pass.
- A rotating head selection avoids starving owners when more owner heads than
  physical slots exist.
- Reclamation protects each owner head; it does not interpret numeric global
  tickets as cross-owner publication order.

Waiting recognizes a ready head in either location, or a successful later job
that can actually fit the retained store. Later unfinished chunks, full storage
and oversized results must not create a busy wakeup. Publication still delivers
chunks before the corresponding completion. Alias publication can advance the
canonical cursor even when no physical slot is free.

## Failure semantics and diagnostics

Cancellation and failures preserve the original eligible owner heads for
cleanup. Later retained results remain explicitly uncommitted and retain their
own partial-initial-overlap scope. They cannot consume another job's frontier
diagnostics, advance past a failed head or disappear through alias publication.

Telemetry now separates occupied physical slots from running workers and total
unpublished jobs. The latter includes retained completions and can exceed the
inspector count. It must not be reported as busy cores. Per-owner occupied-slot
peaks are likewise not a measurement of simultaneous useful CPU execution.

## Validation checkpoint

The clean optimized source-snapshot gate passes **195 tests, zero failures and
one existing ignored diagnostic**. Its 32 owner-focused and two ready-wait
tests overlap that total. All 58 snapshot hashes and 15 owned production-path
hashes agree before/after. The gate includes the explicitly adopted shared store
and Ordered hook, but excludes unrelated worktree formatting and vendor changes.
It uses cached release dependencies and is not the full Cargo/API gate.

Independent source review passes. It identified one allocation-failure corner:
failure to allocate the returned reclaimed-ticket list could leave readiness
true and cause a busy retry. The fix makes optional allocation failure sticky,
without changing slots or responsibilities. A regression tests that fallback
state; it does not claim to force the system allocator to fail.

Other focused regressions cover retained results behind a silent head, retained
chunk-before-completion, full-store waiting, active incomplete streams, entry and
byte caps, cancellation with seven retained partial-overlap responsibilities,
zero-free-slot alias normalization and rotating more heads than inspectors.

Evidence: `TMP/owner-retention-final.wpjcqf/` and independent review
`TMP/owner-retention-independent-audit.fMKunj/REPORT.md`. The earlier 192-pass
snapshot is preserved separately; it predates the fallback fix and three new
tests. The full release gate also passes: **521 library tests, zero failures,
one existing ignored**, with 42 focused owner tests overlapping that total.
Native worker-budget controls execute without preflight-unavailable skips.
Test compilation takes 11 min 35 s, library tests 47.76 s and CLI compilation
3 min 35 s; compilation is not solver time. The gate exits zero at 22:01:57 UTC
after rechecking walking-source hashes. Evidence is in
`TMP/owner-retention-release.Bp5lRC/`.

## Completed matched A11 performance gate

The frozen release executable is
`be31322c2908deca0543bc7a7cd6e44de48e0347078dcbe8ee30b4ae9d731eea`.
Six sequential controls complete using unchanged A11/R2/D9 inputs, four saved
owners and 86 routes. Its four regions represent 45,342 starting tuples;
descendants are not clipped. Policy order alternates Ordered/Owner,
Owner/Ordered, Ordered/Owner. All per-run scope and raw-responsibility checks
pass. The native 50-worker budget uses CPUs 0–49; the observer uses CPU 50.
The supervisor retains 24/32-GB soft/hard supervised-tree memory limits,
500,000-domain and 50-million-event allowances, and no elapsed deadline.
No rules are generated.

| Pair | Ordered traversal (s) | Owner traversal (s) | Ordered native visits | Owner native visits |
|---|---:|---:|---:|---:|
| 1 | 8.328027 | 7.484220 | 27,806 | 33,124 |
| 2 | 8.096085 | 7.375207 | 27,806 | 33,038 |
| 3 | 8.283528 | 7.490670 | 27,806 | 33,012 |

Median traversal is **8.283528 s Ordered versus 7.484220 s owner-local**, about
9.65% less elapsed time for owner-local. These are descriptive medians on a
shared host, not confidence bounds or full-family scaling evidence. The same
post-preparation timer includes initial admission, traversal, publication,
report assembly and queue cleanup; it excludes owner unload/output writing.

| Three-run median | Ordered | Owner-local |
|---|---:|---:|
| Native inspections | 27,806 | 33,038 |
| Events | 695,918 | 762,284 |
| Whole supervised-process CPU seconds | 84.21 | 105.08 |
| Whole supervised-process peak RSS, KiB | 584,444 | 640,288 |
| Corrected sampled mean busy cores | 9.99 | 13.99 |

Whole-process CPU/RSS include setup and teardown. The independent post-run
review identified a reporting-only CPU-window omission: the frozen analyzer
excluded valid `domain_delegated` traversal heartbeats. The separate corrected
analysis includes that event alongside `domain_started` and `domain_progress`.
Only the first Ordered run changes, and its three-run median becomes 9.99 rather
than the original analyzer's 9.78. `domain_draining` belongs to failed cleanup
and occurs in none of these six runs. Original scripts and receipts remain
unchanged; no native rerun or elapsed-time correction is needed. Heartbeat
windows remain approximations, not exact phase CPU.

Owner-local reclaims 25,613–25,869 completed slots per run and retains at most
3,553–3,616 results, with peak accounted storage of 21.68–23.66 MB. Ordered
retains at most 256 and about 6.04 MB. Both report zero producer full-buffer
backpressure, and all physical/retained state drains at completion. Outstanding
job counts above 25 describe retained responsibilities, not extra running cores.

Steering, six raw receipts and analysis are in
`TMP/owner-retention-a11-steering.d7SNxy/matrix/`. Independent review is in
`TMP/owner-retention-measurement-audit.J7APIj/RAW_RESULTS.md`; the raw scope/resource/hash
audit passes with the separate `CPU_WINDOW_CORRECTION.json` described above. The steering
session exits zero. A separate sampling-profile run begins only after all six
runs finish and is not part of these timing measurements.

The historical single-pair baseline was 7.777882 s Ordered versus 9.593036 s
concurrent owner. It used a different binary and Ordered-only reclamation;
the new within-binary repeated comparison is the primary result. Lower wall
time comes with more native work, CPU and memory. This fixes a real slot-use
limitation but does not solve repeated-work amplification or fill fifty cores.
The larger completed control below still does not justify changing the default.
The full 67-owner envelope remains incomplete and stopped; no completion ETA
follows from these pilots.

## Larger A12 control

One sequential Ordered/Owner pair also completes with the same frozen CLI,
four saved owners, 86 routes, worker budget, affinity and resource allowances.
A12/R3/D9 represents 357,192 starting tuples in four regions. All descendants
remain required; their scheduled rank bounds reach four. No root build or
profiling run overlaps these timings. This is one observation per policy on a
shared host, not a replicated result or a cold operating-system-cache test.

| A12 measurement | Ordered | Owner-local |
|---|---:|---:|
| Traversal seconds | 28.426827 | 26.745764 |
| Native inspections | 62,562 | 77,661 |
| Events | 2,507,532 | 2,804,513 |
| Whole supervised-process CPU seconds | 258.47 | 244.01 |
| Whole supervised-process peak RSS, KiB | 1,398,396 | 1,530,548 |
| Corrected sampled mean busy cores | 8.98 | 9.02 |

Owner-local reduces wall time by 5.91% in this pair, with 24.13% more native
inspections and 11.84% more events. Whole-process CPU is 5.59% lower and peak RSS
9.45% higher. Unlike A11, average sampled CPU occupancy barely changes. Do not
infer a utilization breakthrough from dispatch width or more scheduled jobs.

Independent raw review verifies all native, alias and partial-anchor
responsibilities, zero frontiers/errors/pending work, and drained physical and
retained state. Neither run reaches a resource cap, allocation fallback or
producer backpressure. Ordered retains at most 255 finished results (12.71 MB
accounted); owner-local retains 3,871 (47.74 MB accounted). Those bytes are not
total RSS. Initial owner preparation takes 2.772/2.767 s separately from traversal.

Steering and raw results are in `TMP/owner-retention-a12-pair.1yjMtR/matrix/`.
The corrected CPU estimator includes `domain_delegated`; these remain
heartbeat-labelled intervals rather than exact native phase accounting.
All per-run checks and the independent raw audit pass. The scoped coverage
flags deliberately do not claim full-family closure or new IBP generation.

Next reduce repeated native work as well as scheduling overhead. The separate
[CPU profile and native substitution audit](finite_closure_native_profile_2026-09-23.md)
identify a measured algebra hotspot. The
[radical architecture review](finite_closure_architecture_review_2026-09-23.md)
also considers changing the unit of closure work itself.
