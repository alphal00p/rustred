# Runtime ordering and completed S5: measured controls

Status: optimized four-loop S5 comparisons and five ordering scouts complete;
larger five-loop measurements in progress. **No Epoch deployment recommendation yet.**
See [the active plan](../../ASTER_FINAL_PUSH_FOR_ALL_OPTIMIZATION.md) and
[progress log](../../CODEX_PROGRESS.md). Production LC2 was not modified.

## Frozen implementation and measurement boundaries

Implementation `711b18c5` on `fable_5_1_parallel`; subsequent `27b95768` changes
documentation only. Native core/application, installed Python, public CLI and
independent four-/finite-five-loop correctness gates pass. The new executable
uses the same campaign profile as the preserved baseline: optimization level 3,
fat LTO, one codegen unit, no application opt-level-1 override.

| Executable | SHA256 |
| --- | --- |
| Old CLI (`1b33ad29`) | `73253922552ef341e3e97522d9481a4e369d388e9f8612c4ac92468c3c584a14` |
| New CLI | `560f0dddea01e7be22aa7820a2c943a67c1f84b2766c1f6f1a47897867d33420` |
| New normal ordering inspector | `6baeca06d9f1aa55bb7635cda3429a8e346eb9e885dfae48f01e2ac68d954988` |

The new build took 3,898.575 seconds under the resource guard. Compilation is
excluded from the solver measurements. Evidence and exact requests are under
`TMP/aster-integration-20260930-resumed/optimized/FROZEN_BUILD.json` and
`s5-pilot-plan/`, relative to the same local evidence directory.

## Fixed-work S5 comparison

All arms use identical saved rules: 16 owners, 508 routes, 58 required queries,
16 workers, fixed lockstep cut B16, FIFO, snapshot lookup and G2 Union. Affinity
is CPU32–47, separate from LC2 and with no concurrent compiler. Each arm has its
own inclusive 30-minute budget; all eight completed uncensored in under 80
seconds, including verification and orchestration.

`Native` includes process launch through owned-group drain; `Cold` is the separate
guarded full native reinspection. Lock admission and recorder shutdown are
reported separately. The diagnostic Python audit adds about 1.15–1.17 seconds
per arm and remains in the raw receipts. CPU below is waited native child
user+system time. RSS is sampled peak process-tree RSS, in decimal MB.

| Arm | Native s | Cold s | Native+cold s | Native CPU s | Peak RSS MB |
| --- | ---: | ---: | ---: | ---: | ---: |
| Old h0, repeat 1 | 15.308 | 15.182 | 30.490 | 21.635 | 371.5 |
| New h0, repeat 1 | 14.801 | 15.153 | 29.954 | 22.400 | 336.6 |
| New h0, repeat 2 | 14.391 | 13.157 | 27.548 | 21.464 | 351.1 |
| Old h0, repeat 2 | 14.011 | 13.168 | 27.179 | 21.088 | 366.1 |
| New helper control h0, repeat 1 | 15.027 | 13.157 | 28.184 | 21.427 | 344.6 |
| New h2, repeat 1 | 14.051 | 13.157 | 27.208 | 22.555 | 356.2 |
| New h2, repeat 2 | 14.383 | 16.159 | 30.542 | 22.566 | 333.8 |
| New helper control h0, repeat 2 | 14.041 | 13.161 | 27.202 | 21.268 | 358.2 |

Here h0 reserves 15 inspectors plus the coordinator; h2 reserves 13 inspectors,
two preparation helpers and the coordinator. Both stay inside the same worker
budget. The separate ABBA helper block does not reuse earlier measurements.

Every arm cold-verifies all 58 required queries, 32 roots and 31,826 native
inspections. The same 51,166 domains and 1,149,999 dependency edges are retained.
Both old/new pairs have exact durable mathematical-state equality; both helper
pairs additionally match lookup/verification diagnostics. Full typed-record
decoding is not reimplemented by the comparison script: native readers validate
their own records, and native cold reinspection is mandatory. This is scoped
saved-rule coverage, not unrestricted source-IBP certification.

### Interpretation

- **No decisive runtime win.** Old/new median native+cold is 28.834 versus
  28.751 seconds, approximately 0.3% lower; the two paired differences have
  opposite signs. Helper-block medians are 27.693 versus 28.875 seconds. The
  slower second h2 cold run is retained, not discarded after inspection.
- **Publication improves locally.** Coordinator P3 falls from 0.51–0.54 to
  0.20–0.22 seconds; boundary work falls from 0.28–0.30 to 0.15–0.17 seconds.
  However, P2 rises from 1.53–1.59 to 1.72–1.80 seconds. Inspection/wait and
  independent cold checking dominate this small control.
- **Checkpoint storage improves substantially.** Actual checkpoint files total
  about 52.269 MB old versus 20.200 MB new (61% smaller). Record segments fall
  from 42.773 MB to 10.704 MB (75% smaller). These are storage results, not a
  claim of equivalent RSS savings or a scheduler-throughput win.
- **Contention is recorded, not normalized away.** The sparse sampler observes
  0.398–2.445 foreign busy cores on the reserved set, generally from one
  ten-second sample. Small timing differences are inconclusive.

All receipts, coordinator-phase timings and independent checks are retained in
`s5-pilot-plan/FOUR_ALL_RESULTS.{md,json}` and the eight per-arm run directories.
Neither the general improvement threshold nor the stronger 1.5× Epoch-over-Ready
deployment gate has passed on these measurements.

## Runtime source and mathematical-order scouts

These are five **single, unpaired** scouts using the frozen new CLI and normal
inspector above, with the same CPU64–79 allocation and 16-worker budget. Each
regenerates both complete parent downsets (roots1022/511, 314/328 sectors),
stages the same 16 owners/508 routes, and retains all 58 required queries. No
old shards fill generation gaps. The existing native admission checks each
saved owner's actual family, sector and order before the walk.

The comparable total below sums measured generation, staging, admission,
adapter-inclusive walk and cold verification. Compilation and manual gaps
between stages are excluded from that total, but all preparation and gaps are
charged to the separate inclusive 30-minute pilot allowance.

| Candidate | Generation s | Walk + cold s | Comparable total s | Domains | Native inspections | Outcome |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| A0: default source visitation, legacy order | 76.347 | 20.615 | 99.282 | 64,129 | 24,415 | All required queries cold-PASS |
| A1: sparse/coefficient-aware source visitation, legacy order | 70.351 | 14.074 | 86.726 | 38,693 | 15,311 | All required queries cold-PASS |
| B0: A1, explicit default mathematical order | 75.333 | 14.215 | 91.853 | 38,661 | 15,295 | All required queries cold-PASS |
| B1: A1, shared-interface mathematical order | 80.363 | 15.109 | 97.786 | 38,555 | 14,322 | All required queries cold-PASS |
| B2: A1, routing-density mathematical order | See raw receipts | Incomplete | Not a completed timing | 139,530 at stop | 100,739 at stop | Four frontiers; cooperatively stopped |

Walk includes the existing steering/guard overhead; native-only times are also
retained in the receipts. Counts can vary slightly with Ready arrival order;
unlike the fixed-cut S5 comparisons, equal graph identity is not asserted here.
The B0 generation counts and coefficient metadata agree exactly with A1, but
their persisted order identities differ, so artifact byte identity is not claimed.

The promising result remains **source selection A1**, not a broader order:
12.65% lower total and about 40% fewer domains than A0 in this scout. B1 saves
another 6.5% of native inspections relative to A1, but generation overhead
makes its total about 12.75% higher. The B0 total is also higher in this single
run; no representation-overhead or noise attribution has been isolated.
Existing historical paired source-strategy gains remain separate evidence.

B2 produced and admitted its owners, then reached two unresolved predicate
geometries twice, under the R5 and R12 starting queries of owner `0111111111`:
`ExcludedConjunction(batch0, rule11, branch1, ordinal0)` and
`Equality(batch0, rule13, ordinal0)`. The engine explicitly reports no
missing-rule claim and no underlying error. The four-frontier count therefore
does **not** establish four missing IBPs or an invalid integral comparator.
In the owner offsets, only positive-sector coordinates vary and the sole
inactive coordinate is fixed at zero, so lowering numerator rank alone would
not remove these cases. Root authorized stopping only this owned experiment;
it saved and drained normally with 3,492 pending obligations, and no cold
acceptance was attempted. Its 156.212-second native walk is censored.

Evidence: `order-pilot-plans/prepared-v5/` below the same local evidence base,
including frozen descriptors, native commands, complete generation checkpoints,
staging receipts, admitted identities, saved walks and independent cold reports.
There is no production restart recommendation from this unpaired portfolio.

## Next discriminating measurements

1. Replicate the promising source strategy and assess limited five-loop transfer
   with the same optimized executables. Diagnose the actual B2 predicates before
   considering a repair or another mechanistic order. Compare any new comparator
   against strong A1, including regeneration and cold verification costs;
   retain unsuccessful candidates rather than repeatedly trying blind permutations.
2. Run the existing larger finite-five-loop old/new fixed-work control before
   tuning the shared index or preparation grain. Four-loop index candidate
   counters increase, but many count bulk-rejected ranges rather than individual
   checks: actual forward callbacks rise from 440,008 to 599,701. Possible stale
   containment-index entries need measurement, not an assumption of a 3× slowdown.
3. Only then compare the selected current Epoch configuration against current
   Ready, and test useful wider execution. More busy cores are not success if
   they create substantially more domain work. Eventual campaign preparation
   must preserve all 116 required queries and 67 auxiliary helpers, without
   narrowing the frozen request.

The independent opt-level-1 finite-five correctness pair already verifies one
bounded query over 910,957 domains and 743,502 native inspections with identical
helper0/helper2 graphs. Those runs overlapped compilation and cannot supply
production timing evidence or a claim that all five-loop queries close.
