# Five-loop pending-work census and reuse opportunity

This diagnostic examines immutable checkpoint 9 from the ongoing campaign,
saved at 04:20:30 UTC on September 25. It does not alter the running job, import
rules, certify closure, or estimate the number of future native inspections.

## Queue composition

A streaming read of the checkpoint's domain-array prefix reconciles exactly
with its published metadata: 25,410,742 admitted logical IDs, publication
watermark 14,176,405, and 11,234,337 pending IDs. These pending IDs **include
delegated aliases and reserved work**. The ledger was not decoded; an ID count
is not a remaining native-work count or a count of distinct scalar integrals.

| Pending phase | Logical IDs | Meaning |
|---|---:|---|
| Route | 8,849,671 | Transport to an installed owner, not necessarily an expensive rule application |
| Apply | 2,384,666 | Apply the owner's saved rules to a symbolic domain |
| Total | 11,234,337 | All unpublished logical IDs, including aliases |

There are 7,006 phase/owner groups and 6,983 distinct masks. Most masks are
routing destinations, not additional top-level input families. The largest
phase/owner group has 351,445 IDs (3.128% of the total). The Apply owner that
dominates the recent slow-head samples, `011101110111000`, has 132,278 IDs:
1.177% of the whole queue, or 5.547% of pending Apply IDs. Neither percentage is
a fraction of CPU time or future native work.

Support sizes eight and nine together account for 6,117,314 pending IDs. Every
stored rank cap is finite, up to 21; 1,525,078 IDs have a cap above the initial
rank 15. The largest stored finite A cap is 31. Another 2,082 records have no
explicit A cap; coordinate bounds can still bound their A, so this is not a
count of mathematically unbounded regions. Descendants have not been clipped
back to the initial A/R restrictions.

The scan reads 5,747,923,557 bytes of the checkpoint prefix in 120.592 seconds,
using one non-production CPU and 52,264 KiB peak RSS. It never materializes the
full checkpoint or queue. Twenty small framing/count tests and an independent
aggregate audit pass. This is a diagnostic prefix read, not full checkpoint
validation or a replacement for the native restore path.

Evidence: `TMP/five-loop-pending-census.lOnGzu/`, including `scan.py`,
`result.json`, progress output and `INDEPENDENT_AUDIT.md`. Checkpoint 9's
recorded full size is 16,792,184,119 bytes. The native campaign is unchanged.

### Later live observation

A separate bounded monitor read at 04:55:59 UTC records 7.194 million native
inspections, 11.522 million pending logical IDs and 106.588 GB RSS, with no
reported frontiers or worker faults. During its preceding 25-minute window,
native completions increase by 84,082 but pending work increases by 296,691.
Actual average CPU usage is 2.172 cores; measured preparation plus commit spans
12.80% of coordinator elapsed time. The latest five minutes are slower:
485 native completions, 1,160 more pending IDs and 1.798 busy cores.

The same slow Apply owner accounts for 297 of 298 recent source-head samples,
and finished held work averages 166.1 entries in that five-minute interval.
These source samples are not CPU attribution. Checkpoint 9's save predates
the observation window. This is consistent with an inspection/ordered-head
bottleneck following a faster burst, not an established convergence or
nontermination result. Fifty reserved workers do not imply fifty busy cores.
No completion ETA follows from this evolving worklist. The independent
arithmetic and process-identity audit passes; evidence is in
`TMP/five-loop-grain-wait-snapshot.DvpKwy/`.

## Exact slow-head geometry

The scan also recovers historical ID 14,170,851, the head captured by the
04:09 read-only profile. It is already below checkpoint 9's watermark; it is
not claimed to remain pending at that checkpoint.

Its owner has nine active denominators. In local coordinates all lower bounds
are zero. Two active coordinates can be 0 or 1, the other active coordinates
are fixed to zero, and the six inactive coordinates can range from 0 to 18.
The preserved joint predicates are R <= 18, A <= 11 and D <= 11, with D=A-R.
Thus 9 <= A <= 11. These are aggregate domain bounds, not one integer integral.

An input-only replay with the existing normal-release helper now completes
three calls per mode in opposite orders: Off/Refined/Off and
Refined/Off/Refined. Both fresh processes load the same 67 owner programs once.
The live walker can first remove an initial high-D overlap, so whole-parent
timing must not be presented as the exact live residual's timing. Results
remain separate from recursive traversal and from the 24-helper insertion-grain
microbenchmark.

| Whole-parent native metric | Refinement off | Refinement on |
|---|---:|---:|
| Median wall time | 44.42409 s | 44.42568 s |
| Median process CPU time during native call | 44.09 s | 44.02 s |
| Selected pieces | 1,035 | 1,035 |
| Shift groups | 894,794 | 894,794 |
| Emitted successors | 870,691 | 914,467 |
| Term visits | 925,633 | 969,928 |
| Refinement steps / cells | 0 / 0 | 44,295 / 88,590 |

All same-mode non-timing receipts repeat exactly; native/callback errors,
uncovered problems and unsupported successors are zero. Full-helper peak RSS
is approximately 5.99 GB. Whole-process times of 149.36/150.79 s include three
native calls and 14.23/14.25 s loading, and are not the per-call timings above.
An independent raw-receipt audit passes.

This head does **not** reproduce the older head's local refinement benefit.
The observed median wall change is +0.0036%, with 5.03% more successor work.
This small same-host experiment establishes no useful speedup here, and the
option remains off by default. Evidence:
`TMP/current-head-refinement.DxFsQ4/`, including `INDEPENDENT_AUDIT.md`.

## A stronger reuse cut is possible without new case geometry

The actual immutable initial prefix contains two same-owner obligations:

| Initial ID | A bound | R bound | D lower bound |
|---|---:|---:|---:|
| 66, required input | 24 | 15 | 9 |
| 133, auxiliary cover | 24 | 15 | none |

Both are real pinned responsibilities, not declarations that their descendant
work is finished. The existing `InitialOverlapIndex::plan` uses the first
successful stored anchor cut. Here ID 66 permits removing D >= 9 and leaves
D <= 8 for native inspection. ID 133 has a derived minimum D of -6, but its
high slice at that cut is not contained: A=11, R=17, D=-6 is a counterexample.

However, using ID 133 with a **query-dependent cut D >= -4** is valid:

    R = A - D <= 11 - (-4) = 15,
    A <= 11 <= 24.

All remaining coordinates lie in that auxiliary anchor's full orthant. The
complement is exactly the same parent with D <= -5. The cut is tight for this
single anchor: D=-5 admits A=11, R=16, which is outside the anchor. The current
planner neither searches this cut nor compares it with the first valid one.
An independent source/mathematical audit confirms this example.

A generic candidate improvement is to search nested high-D slices using the
existing exact `DomainPowerSummary::contains` service and choose the smallest
proved cut, with a stable anchor-ID tie break. No new rational reconstruction,
polynomial kernel, affine case format or topology-specific dispatch is needed.
Finite checked endpoints, cancellation, empty slices, malformed input and
overflow must retain the current safe fallback. Initial obligations must still
be inspected; every reused slice must retain its actual pinned anchor and the
residual must still finish. Changed event streams need checkpoint-prefix and
policy-compatibility tests.

### Native residual comparison

The input-only comparison now completes four fresh processes in the fixed
order current/adaptive/adaptive/current. Each uses the unchanged normal-release
helper's Off/Refined/Off diagnostic sequence. The primary comparison below uses
the mean of the two Off calls **within each process**; those repeated calls
are not treated as independent trials. Both pairs use the same historical
parent, owner programs and native policies, but the residual workloads differ
by their exactly justified D cut.

| Opposite-order pair | Current residual D<=8 | Adaptive residual D<=-5 | Current / adaptive |
|---|---:|---:|---:|
| Current first | 45.5823 s | 40.8602 s | 1.1156 |
| Adaptive first | 47.1558 s | 40.0543 s | 1.1773 |

The corresponding CPU summaries are 45.22/40.53 s and 46.73/39.73 s.
Every same-query non-timing receipt repeats exactly across fresh processes.
The Off successor count falls from 870,381 to 735,519 (-15.49%), shift groups
from 894,462 to 755,341 (-15.55%), and selected pieces from 1,027 to 874.
All four helpers finish without native/callback errors or reported uncovered
problems/unsupported successors. Their sampled aggregate peak RSS is about
5.99 GB. Initialization takes 13.68–15.68 s per process and is excluded from
the native timings. Both cases retain their coefficient/materialization limits;
this diagnostic does not retain a coefficient-payload equality certificate.

The two secondary refined calls per residual also complete, but do not justify
combining refinement with the reuse change: the earlier whole-parent study
finds no benefit for this head, and refinement increases successor counts.
Keep that policy off. All diagnostic runs use the existing CPU56–61 RAM-only
guard, one native inspector and one-thread inner pools, with no time limit.
Checkpoint 9 remains unchanged in all before/after live brackets. The active
production job and the separate test build make this a shared-host comparison,
not a dedicated-machine statistical estimate. For example, the last control's
two Off calls take 49.47 and 44.84 s. There are only two paired process
comparisons; no confidence interval or general speedup claim is justified.

The result is a **10.36–15.06% decrease in local native residual wall time**,
not a parallel speedup, new IBPs, whole-parent elimination, recursive campaign
speedup or closure. The reused high-D slice remains an actual pinned initial
responsibility, including all of its descendants. Evidence:
`TMP/adaptive-overlap-native.PEKZWH/`, with the fixed plan, both query inputs,
six input-regression tests, full mode receipts and `analysis.json`.
An independent source, mathematical and raw-receipt audit passes.

The generic adaptive planner remains a design, not production code. A scoped
implementation is justified for further testing, but must measure full
descendant traversal and lookup overhead before promotion. It cannot fix the
separate ordered-publication bottleneck. The admission-grain experiment and
Ready-publication recovery/completion gates remain separate workstreams.
