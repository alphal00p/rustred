# Epoch P1 witness and compact source-row comparison

The optimized four-loop ABBA control passes all native cold checks but is
performance-neutral. The first matched, bounded five-loop pair also completed:
P1 validation is cheaper, but larger pending work and unchanged utilization
prevent a closure-speed claim. These are **old Epoch versus new Epoch** measurements,
not evidence of superiority over Ready or of five-loop closure.

## Intervention and controls

The package removes repeated same-input P1 union checking through a private
single-use witness and transports verified P2 Stored rows without reserving a
full candidate payload for each row. The optional repetition observer is off.
It does not change rules, request scope, containment authority or publication
policy. Individual changes are not isolated by this comparison.

- Old source `d55cfb1c`, CLI SHA256
  `23d836d7b05fac6b007cc1adea84f04548d5bf0e0a94a28ca877f98942da8045`.
- New source `58e63614`, CLI SHA256
  `0f2536a7127c5474189f93cac7db7bb62f4b076140256f4b5ab7ab3c77b75c70`.
- Both use opt3, fat LTO and one codegen unit, with no app-opt1 override.
  The new build completed in 3,871.496 seconds, excluded from run timings.
- Identical saved A1 four-loop inputs: 16 owners, 508 routes, 58 required
  queries, 32 roots. No generation, input reordering or resume is included.
- W16 on CPUs 32–47: 15 inspectors, no preparation helpers, one coordinator.
  FIFO, oldest-prefix, window 76, cut 16, 1,024 extra result slots, 256 MiB
  soft returned-result admission, immutable snapshot lookup and G2 union.
  `RUSTRED_EPOCH_PROFILE=0`; nested compute pools remain capped at one.

Each arm includes a fresh-process cold-All verification. Primary wall time is
native launch through owned-process drainage plus the cold-verifier guard.
The inclusive controller additionally counts metadata, admission, recorder
shutdown and secondary transport checks. All arms finished uncensored within
their 30-minute inclusive ceilings. No production process was changed.

## Four-loop result

Times are seconds; RSS is the sampled native-process-tree peak, not an exact
allocation measurement. Execution order was old1, new1, new2, old2.

| Arm | Native | Cold-All | Primary | Inclusive | Traversal | Native CPU | RSS MiB |
|---|---:|---:|---:|---:|---:|---:|---:|
| Old 1 | 7.558 | 9.140 | 16.698 | 22.694 | 3.758 | 22.792 | 199.31 |
| New 1 | 7.157 | 9.140 | 16.297 | 22.335 | 3.779 | 22.718 | 203.75 |
| New 2 | 6.761 | 8.137 | 14.898 | 20.207 | 3.766 | 22.237 | 202.25 |
| Old 2 | 6.750 | 8.136 | 14.886 | 20.107 | 3.758 | 22.391 | 211.70 |

Median primary time is 15.792 → 15.598 seconds (−1.23%), but the pair changes
are −2.40% and +0.08%. Traversal is 0.39% slower. This is not a demonstrated
speedup or campaign-switch justification. Median P2 time is 0.82234 → 0.81647
seconds (−0.71%); its nested source-resolution component is 0.53419 → 0.52581
seconds (−1.57%). P1 changes 0.06581 → 0.06556 seconds. Smaller row-buffer
capacity is not a measured whole-campaign memory saving.

Every arm has the same 26,025 domains, 17,957 native inspections, 495,898 edges,
872,486 events, 731,181 accepted P2 rows, 36,410 source tasks and 18,226 waves.
All 58 required queries and 32 roots pass cold-All with all recorded natives
re-inspected and zero violations, frontiers or remaining work.

Geometry, ledger, ordered dependencies, anchors, flags, owner/query scope and
canonical logical-record/edge digests agree. The first raw strict comparator
nevertheless remains **false**: the diagnostic `walk.miss_requests` changes
140,317 → 140,318, matching one fewer inspector Stored hit and corresponding
lookup/verification accounting. Unchanged resolver code and existing snapshot
tests support timing-sensitive snapshot selection as the explanation; the
triggering row/version was not traced. The second strict comparison is true.
Full typed-record byte equality was not compared. Do not erase the first
comparator result or describe all stored metadata as identical.

The native checkpoint-only summary exits 4 and the secondary Python transport
check remains `INCOMPLETE`. Neither is relabelled as closure: acceptance comes
from the separate raw native cold-All reports. All 32 recorded owned process
groups and the outer drivers drained before resource release.

Native runs are shorter than the existing ten-second foreign-load recorder
interval, leaving no samples. Foreign load and steady core utilization are
therefore unknown, not zero. Both production campaigns continued on disjoint
cores; no concurrent root build ran. This is not a quiet-host measurement.
Peak pending and full outer-adapter CPU are unavailable in these receipts.

## First bounded five-loop pair

The first fresh old/new five-loop h0 pair finished its bounded protocol, using the same
67 owners, 8,246 routes and 183 ordered queries (116 required, 67 auxiliary),
W32 on CPUs 0–31. It used the existing 20-minute cooperative native window
inside a 30-minute inclusive setup/checkpoint/cold-check/drain ceiling.
No hard stop occurred. The reached graph prefixes differ; these are not
completion times for the same finished solve.

| Metric | Old | New | Change |
|---|---:|---:|---:|
| Inclusive stopped-run/check/cold protocol, s | 1,419.26 | 1,429.54 | +0.72% |
| Native traversal, s | 1,136.37 | 1,138.67 | +0.20% |
| Native inspections | 3,423,280 | 3,718,128 | +8.61% |
| Committed domains, including aliases | 6,224,002 | 6,660,448 | +7.01% |
| Scheduled domains | 8,910,219 | 9,646,709 | +8.27% |
| Pending domains | 2,686,217 | 2,986,261 | +11.17% |
| Registered native + supervisor CPU, s | 6,417.74 | 6,432.85 | +0.24% |
| Mean observed native cores | 5.322 | 5.325 | essentially unchanged |
| Sampled guarded-tree peak RSS, decimal GB | 14.893 | 15.170 | +1.86% |
| P1 validation, s | 97.796 | 63.404 | -35.17% |
| P2 total, s | 307.351 | 323.098 | +5.12% |
| P2 source resolution, s | 217.878 | 225.528 | +3.51% |
| P2 cost per accepted row, ns | 1,746.78 | 1,751.05 | +0.24% |
| Source cost per accepted row, ns | 1,238.27 | 1,222.26 | -1.29% |

The P1 reduction is the strongest positive signal: its wall time fell despite
about 11.5% more logged G2 records. Per-record/covered-point proxies improve
about 42%, but neither is an identical-work denominator or isolates all P1
work. This combined-build comparison does not isolate each change causally.
Compact P2 transport has no clear timing win here. Inspection throughput per
traversal second improves 8.39%, but the larger pending graph prevents treating
this as reduced total work or faster closure. The carried exploratory screen
of at least 10% more throughput together with lower P2 cost per row is not met.
There is no utilization breakthrough, repeated-pair evidence or switch advice.

Both arms saved resumable CP6 generation 1, admitted all 183 queries, and had
zero errors/frontiers/abandoned obligations. Both exercised ten safe
undecided-union fallbacks. Their fresh-process cold checks deliberately used
**None**: zero native reinspection and zero independently verified roots,
raw `INCOMPLETE`/exit 9, with no reported structural violations. No actual
resume, scoped closure or completed-family claim is made. All eight recorded
owned process groups and outer adapters drained before the next build.

CPU totals above exclude the outer adapters; there is no all-process CPU
comparison. Production continued on disjoint cores but still represents host
contention. Both effective RAM guards stayed far above measured use. This is
one old/new pair, not a quiet-host confidence interval. The next suffix-lookup
implementation is not part of either binary or these results.

## Evidence

Local raw receipts, exact commands, input hashes, per-arm clocks, frozen build
identity and the independent review are retained under
`TMP/postlaunch-20260930/compact-source-rows/performance/` and
`TMP/postlaunch-20260930/p1-compact-optimized-bin/`. The complete local table is
`FOUR_RESULTS.md`; raw state comparisons are `four/pair-r{1,2}-state.json`.
The five-loop report, arithmetic and drain receipts are `FIVE_RESULTS.md`,
`FIVE_PAIR_RESULTS.json` and `FIVE_DRAIN.json`. Its raw arm results are under
`five/{old-r1,new-r1}/h0/`; the independent interpretation audit accompanies
the report. Campaign outputs and executables remain untracked.
