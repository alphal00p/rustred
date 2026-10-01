# Apply exact prepared retirement IDs once

Status: source implemented and independently reviewed on October 1, 2026.
Native differential and application tests pass. Repeated combined four-loop
controls also pass, but show no traversal improvement. A representative
large-index five-loop comparison remains pending. Production is unchanged.
The implementation is parked in the isolated
`TMP/prepared-retirement-20261001` worktree. Its three files were confirmed
byte-identical before removing only this unpromoted patch from the `main`
working tree, so new rule-selection experiments use the unchanged scheduler.
The documentation/test milestone does not deliver this engine change.

## Repeated work being removed

Epoch P2 computes the sorted set of live domain IDs contained by each proposed
survivor. P3 previously traversed the containment index again, applying coordinate,
word and lane filters before asking whether each surviving ID was in that set.
The second pass does not discover a new relation: its final predicate is simply
membership in P2's already prepared set.

The saved same-start five-loop continuation attributed 73.44 seconds to P3 out
of 838.68 seconds of restore-inclusive traversal. A later production window
attributed 19.19% of coordinator wall time to P3. An earlier sampled profile
contained reverse-index ancestry in 38 of 86 P3 samples. These different
observations motivate investigation; they are not interchangeable timing scopes
or a predicted speedup. Even eliminating all of P3 could save no more than its
share of the relevant whole run.

## Implementation and invariant

`AggregateIndex::retire_with` owns the existing group traversal, slot retention,
stable block compaction, insertion-tail pinning, group removal and accounting.
There are two private ways to select the slots it removes:

- The existing geometric path retains its filters and predicate callbacks.
- Epoch's new `retire_ids` intersects a sorted prepared set with the **current
  live IDs** in each block. It uses ID bounds, not another containment test.

Let `R` be the exact P2 retirement set and `L` the IDs still live when P3 reaches
this survivor. The required removals are precisely `R ∩ L`. Earlier publication
within the same cut may have removed some overlapping IDs; those no longer
occur in a live block prefix. Newly inserted survivor IDs are outside `R`.
The new path never retains a snapshot's physical block or slot addresses.

Every ID in `R` passed the original exact P2 containment check against the same
immutable domain image. Therefore the old necessary geometric filters cannot
exclude any still-live member of `R`; all other IDs fail its final membership
test. Both paths must consequently produce identical slot masks. Applying them
through the shared mutation code must preserve full index layout and storage
accounting, not merely the final set of live IDs.

Important boundaries:

- The prepared sets are private current-P2 plans, not an external assertion or
  reusable certificate. This adds no containment or algebra primitive.
- P3 retains its old-ID bound, current-live expected count and poison-on-count-
  mismatch checks, as well as the existing verified transfer targets.
- Every signature-eligible group is still traversed even for an empty set.
  Restored empty blocks must be removed as before; a reserved insertion tail
  must remain. An empty set is not permission to return early.
- Group ID ranges can interleave. Each block searches the sorted set separately;
  no monotone cursor is carried between groups. Only live slots participate.
- Ready and initial admission retain the geometric path. Epoch's legacy lookup
  restore also calls the survivor path with an empty set and is in test scope.
- Index diagnostic filter counts can differ. Canonical IDs, dependency edges,
  retirement counts, checkpoint images and publication decisions must not.

## Validation before acceptance

The existing 8,000-operation historical-layout test, 6,000-operation storage
test, native-summary reuse test and restored-empty-block test now run both
paths. They compare full serialized checkpoint index images (including stale
tail slots, envelopes and physical order), runtime kernel images, positions,
IDs and storage totals after mutations. An additional test exercises overlapping
sets, shifted live slots, interleaved groups and newly appended IDs.

These tests supplement, not replace, the existing independent historical layout
model. Full queue, Epoch publication, admission, cancellation and restore tests
must pass, including fixed-publication/helper-budget graph equivalence.
Independent source review passed. The optimized application suite passes
1,320 tests (14 explicit external/scale tests ignored), including all five
modified index controls, lookup restoration and native CP6 publication/replay
controls. Its 16-core allocation skipped ten genuine 50-worker test arms;
those wider runs remain unvalidated by this receipt. Focused passes are subsets
of that suite, not additional test coverage. Evidence:
`TMP/postlaunch-20260930/rule-quality-native-20261001/app-full/`.

Then compare optimized binaries on identical saved four-loop inputs and the
existing bounded five-loop control, charging complete preparation, publication,
checkpoint and verification costs. Keep the generated-rule portfolio comparison
separate: changing both rules and publication in the same pair would confound
the result. Local mask timings or busier cores do not establish a campaign gain.
Report preparation and cold verification separately from native traversal.
For the long production campaign, a sustained traversal/work reduction may
outweigh a one-off setup cost; setup alone is not a veto. A local phase speedup
still cannot substitute for measured useful traversal or offset extra domain
work without accounting for that tradeoff.

Falsifiers are any changed semantic or physical checkpoint image, missing
retirement, altered publication choice, or material whole-work regression.
Sorted-set lookup may lose to cheap geometric rejection on some distributions;
that possibility must be measured, not dismissed. No production switch follows
from source approval alone.

Implementation author: `frontier_probe_runner`. Independent source reviewer:
`rule_quality_audit`. Root owns integration, resource coordination and acceptance.
The isolated source is retained at `TMP/prepared-retirement-20261001/`;
native evidence is recorded in `CODEX_PROGRESS.md`.

## Completed matched four-loop comparison

The six fixed-rule controls use the already source-replayed A1 programs:
16 owners,508 routes,58 required queries and32 initial root domains. Both
executables use Cargo's ordinary optimized release profile, with16 workers on
CPUs32–47,15 inspectors and inner pools capped at1. Old CLI SHA256`4e76707b…`;
new CLI`d7a21923…`. No rules were regenerated. The new optimized build took
814.51s, excluded from solver timings. Foreign host contention was not measured.

| Policy/build | Native traversal, seconds (two runs) | Domains | Native inspections |
| --- | ---: | ---: | ---: |
| Epoch, previous | 4.290 / 4.255 | 26,025 / 26,025 | 17,957 / 17,957 |
| Epoch, prepared-ID retirement | 4.332 / 4.323 | 26,025 / 26,025 | 17,957 / 17,957 |
| Ready, new build | 3.236 / 3.321 | 39,006 / 39,599 | 15,351 / 13,886 |

Every arm subsequently cold-reinspected all natives and verified all58 queries
and32 roots. Ready's first native run completed normally, but the diagnostic
reader rejected its39.3MB result at a32MiB limit. A reader-only128MiB allowance
and separate cold check qualified the existing result; the native run was not
retimed. Its original failed harness receipt is retained. These are checks of
scoped dependency coverage, not a global termination or unrestricted-family
certificate.

Native preparation was1.099–1.305s. Full arm wall, including staging, native
execution, checkpoint/drain and cold checking, was16.30–18.66s; the first Ready
arm's16.792s is segmented charged work, not a continuous stopwatch. Native walk
phase CPU was22.64–27.13s, sampled peak memory219.1–247.6MiB. The full per-arm
costs and boundaries are in the local `RESULTS.md` referenced below.

Median-of-two traversal rises1.29% with the change (4.272→4.328s). P3 phase wall
is0.10386/0.10052s old versus0.11640/0.10339s new. This supplies **no demonstrated
speedup**, even locally. Lower inclusive wall for the new Epoch arms comes from
the cold-verifier phase, not sustained traversal improvement. Ready is roughly
24% faster on this control's median traversal, while selecting a different
graph. Neither result forecasts performance on a mature five-loop index.

### Precisely qualified state equivalence

The unchanged checkpoint comparator retains raw differences. Both pairs have
snapshot hit/miss accounting differences; in the first pair13 missing snapshot
targets become13 additional current-store lookups. Existing differential tests
already distinguish this accounting from selected mathematical work.

The second pair also has two different persisted G2 dispatch versions: nodes
21105/21126 change809→811 and809→812. All49 anchor records otherwise match byte
for byte, including IDs, order, scopes, lender stamps and residuals. Both
histories satisfy the authority inequality: latest lender664 precedes dispatch,
which precedes the unchanged recipient merge epochs815/817. The reviewer checked
the source validation and exact bytes; these are valid differing snapshot
provenance, not changed selected coverage. They must not be broadly erased as
irrelevant timing. All other compared graph/index/ledger/edge sections agree.

Accordingly, selected mathematical state and anchor coverage are qualified;
**full durable checkpoint or typed-record equality is not claimed**. The saved
`records_digest` hashes ID/tag/outdegree, not whole record bodies, and alone
would not resolve this difference. No comparator exemptions were introduced.

Evidence:
`TMP/postlaunch-20261001/prepared-retirement-performance/RESULTS.md`, its six
arm receipts, and raw `receipts/compare-r1` / `compare-r2` reports. Runner:
`frontier_probe_runner`; independent classification: `rule_quality_audit`.
The change remains unpromoted; no production restart follows from these
four-loop results. A later private five-loop baseline saved successfully after
900s; the user then prioritized all resources for rule optimization, and its
owned cold check was interrupted cooperatively. The candidate was never run.
This incomplete pair supplies no performance acceptance. Original checkpoints
and production were untouched; raw evidence remains under `five-resume/`.
