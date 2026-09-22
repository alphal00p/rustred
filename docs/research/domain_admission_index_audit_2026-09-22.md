# Independent aggregate-index implementation audit

Date: 2026-09-22. Scope: the aggregate-filtered retained-domain index and its
integration into the shared-owner admission queue. Source review, the focused
optimized gate and saved-descriptor timings are complete. The full application/
Python gate and a matched production campaign remain separate pending gates.

## Verdict

**Source/mathematical audit and the focused optimized gate pass.** No
false-negative aggregate filter, changed successful containing-ID selection,
lost pending obligation or new fallible operation after index retirement was
found in the reviewed slice. Two index-only receipt replays show reproducible
speedups below; neither establishes a production-campaign improvement or
five-loop closure.

Reviewed implementation:

- `crates/rustred-app/src/application/routed_campaign/walking/queue.rs`
- `crates/rustred-app/src/application/routed_campaign/walking/queue/index.rs`
- The new index unit tests and queue `aggregate`/`linear_semantic` test modules.
- Existing maximal-candidate, semantic and finite-comparison-cap tests.

The comparison reference is the preceding linear semantic queue frozen at
`TMP/affine-support-gate.cv24Y0/linear-semantic-queue.rs`. Native geometry is
unchanged; the review also checked the public `DomainPowerSummary` extrema and
containment implementation against the three selected signature fields.

## Correctness invariants checked

**Filtering.** A signature uses exact native A maximum, R maximum and D minimum,
not raw optional input labels. Each forward test is literally a necessary
component of native inclusion: a container needs no smaller A/R maxima and no
larger D minimum. Passing these tests still requires native exact inclusion.
Reverse retirement exchanges container and candidate in the same predicate.
The phase/owner partition remains outside this filter.

**Empty and unbounded domains.** Empty summaries have an explicit signature:
every container may contain an empty query, while an empty container cannot
contain a nonempty query. Upper positive infinity and lower negative infinity
use distinct typed variants. Finite u128/i128 extrema are not narrowed or
replaced by sentinels; u128::MAX and i128::MIN remain finite values.

**Determinism.** Exact-key and dominant-orthant lookup retain their previous
priority. The fallback searches ascending IDs within each eligible group and
returns the minimum successful ID across groups. Consequently group order,
hash allocation and swap-removal cannot change the selected retained container.
The native summary remains the final authority, including for empty queries.

**Retirement.** Only lookup membership changes. Historical domains, exact-key
IDs, summaries, FIFO jobs and the orthant pointer remain intact. Candidate
groups maintain unique membership and ascending IDs. Removing a group repairs
the moved group's position map; the insertion signature's reserved group is
kept even if retirement empties it. The new containing representative is
inserted before successful admission returns. A pending representative remains
an obligation, not proof that the covered region has already been processed.

**Failure atomicity.** Fallible domain/index/group/ID reservations and checked
maintenance-accounting preflights precede retirement and publication. Capacity
and work counters may change on refusal; logical membership and pending work
must not. Retirement and insertion require no further fallible collection
reservation. The eligible-group candidate count bounds every actual reverse
comparison and retirement increment. Ordinary allocator abort behavior is not
newly made recoverable by this change.

**Finite comparison caps.** Explicitly capped queues continue using the raw
historical vector scan, without summary construction or aggregate retirement.
The unlimited lane intentionally performs fewer authoritative comparisons;
counter values, and the point at which a synthetic usize overflow is reached,
are therefore not required to match the old linear scan. A refusal still must
leave logical state unchanged.

## Test coverage and optimized focused gate

The new differential test retains all 46,080 generated proposals, including
exact duplicates, rejected-by-containment requests and empty domains. It runs
the complete stream in forward, reverse and deterministic shuffled order,
checking exact `(ID, admitted)` results and every domain/summary/exact-key/
candidate/orthant state after each request against an independent pre-index
linear semantic policy model. Neither model discharges pending jobs.

Additional tests cover native-summary filter implications, explicit infinities
and wide aggregates, group swap-removal, minimum-ID ties, same-signature group
replacement, reverse-filter exclusions, empty groups, allocation-checkpoint
refusals, count overflow and later reuse after admission errors. Existing test
changes are limited to reading the split index representation and accounting
for comparisons deliberately eliminated by the filter. Finite-cap behavior
assertions remain intact.

An actual-source harness, compiled with `rustc --test -O` against the already
gated release native dependencies, passes **36 tests, zero failures, one ignored
input-driven diagnostic** in 1.91 s. This includes all tests above. The ignored
legacy receipt-replay diagnostic is separate from the production-source timing
harness below. This was not a replacement for the full application build.

A subsequent correctness/counter rerun confirms the complete-stream results:

| Proposal order | Proposals | Admitted | Reused | Linear full checks | Indexed full checks | Group visits / rejected |
|---|---:|---:|---:|---:|---:|---:|
| Forward | 46,080 | 298 | 45,782 | 75,484 | 62,866 | 144,250 / 20,610 |
| Reverse | 46,080 | 26 | 46,054 | 46,072 | 46,046 | 46,094 / 30 |
| Deterministic shuffle | 46,080 | 99 | 45,981 | 47,297 | 47,608 | 53,574 / 1,701 |

Every returned ID and complete admission state match after every proposal.
Importantly, the shuffled case performs **311 more** native checks with the
index. A later-ID group can be visited before an earlier containing ID is found;
global-minimum selection preserves the answer, not necessarily the old number
of checks. Neither comparison reduction nor acceleration is universal.

This generated-stream differential is stronger than replaying only saved
completed domain descriptors, but it is **not** a recording of every proposal
from the real five-loop pilot. The full application gate and matched production
pilot remain necessary. Focused evidence is under
`TMP/aggregate-admission-gate.HrfYdG/`, including `queue-test.log` and
`complete-stream-detail.log`; the latter rerun is not used for timing claims.

## Independently verified descriptor-replay measurements

The standalone `compare.rs` harness compiles the actual frozen preceding queue
and the actual new queue in one optimized non-test executable. The frozen queue
matches the corresponding source in commit `38e30d5` byte-for-byte. The saved
`build-compare.sh` uses `rustc --edition=2024 -O` and release dependencies;
`compare-provenance.sha256` verifies the harness, executable, old queue and new
queue/index source. Executable SHA-256:
`4dcc8e5d66766a21e3db2b0140a474e5900e2faf295717d67e3dbc108a2df451`.

Each input is the completed-domain descriptor sequence from an already stopped
pilot, excluding its final incomplete record. Parsing preserves support-bit
order and all coordinate/rank/A/D fields. It does **not** recreate rejected
original campaign proposals, the unfinished queue or native IBP operations.
Each timing includes coordinate-vector cloning, native summary construction,
lookup/group/hash work, exact comparisons, retirement and admission. JSON
parsing, setup of empty queues, final equality checks and queue destruction are
excluded equally. The first queue is destroyed before the second run; its
returned-ID vector remains for comparison.

There are six completed paired runs per receipt, alternating three old-first
and three indexed-first runs, pinned to physical CPU 0. No root-owned build
overlaps these measurements. CPU time is the calling thread's Linux schedstat
runtime, appropriate for this single-thread index replay, not aggregate campaign
CPU. Parsing is outside both measured intervals. No solver is launched.

| Saved descriptor sequence | Support-aware pilot | Semantic-only pilot |
|---|---:|---:|
| Completed descriptors replayed | 63,913 | 125,503 |
| Linear median wall | 4.879403 s | 8.852455 s |
| Indexed median wall | 1.923161 s | 4.226046 s |
| Median paired wall speedup | 2.588805x | 2.086627x |
| Paired wall ratio range | 2.409748–3.064647x | 2.024547–2.193009x |
| Linear / indexed median CPU | 4.841348 / 1.907155 s | 8.779038 / 4.191961 s |
| Median paired CPU speedup | 2.591477x | 2.086228x |
| Linear / indexed full comparisons | 431,212,172 / 113,846,807 | 791,007,412 / 218,506,452 |
| Included linear / indexed maintenance | 215,606,086 / 49,108,991 | 395,503,706 / 52,695,413 |
| Final candidates, equal in both implementations | 37,018 | 40,037 |
| Retired candidates, equal in both implementations | 26,895 | 85,466 |

Every proposal returns the identical `(ID, admitted)` pair in all twelve runs;
admission, candidate, retirement, summary-build and comparison counts are
deterministic across repeats. All descriptors in these two selected sequences
are admitted in both replay implementations. The median paired ratio is not
the ratio of separately computed medians. These are repeated shared-host
microbenchmarks, not a confidence-bounded full-campaign performance guarantee.

Inputs are `shared-owner-campaign.ujyk56ua/result.json` and
`shared-owner-campaign.r8txq1a3/result.json` beneath
`TMP/correlated-routing-pilots.jw6Bwo/`. The former hashes to
`3a58a9483ecc8b8599b2bb0ebb1749b04bf3a52e9728b47cdab2f1e344b07c39`;
the latter to
`bfc5b16ef5c8956dbe3349bd7213970e5c9d43578db9f2353af93867f15a96c6`.
All twelve JSON timing receipts are in the gate directory. No full-envelope
ETA or campaign speedup is inferred from these index-only workloads.

## Performance interpretation

Group probes are counted only in test instrumentation, including the forward,
maintenance-preflight and actual retirement scans. No production scheduler or
progress schema is changed. Wall/CPU timing must include all group scans,
hash/group maintenance and summary construction; fewer native comparisons alone
are not a speedup.

The preceding support-aware routing run demonstrated why this distinction
matters: equal-source routing fanout fell while global admission throughput
worsened. For this index-only slice, successful domain IDs and descriptors
should instead be identical to the linear semantic baseline. A matched
completed-prefix comparison may be reported only after verifying that equality
(excluding per-record timing); stopped prefixes cannot establish full-envelope
closure or a defensible completion ETA.
