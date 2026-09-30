# Runtime ordering and completed S5: measured controls

Status: optimized fixed-work S5 comparisons, the first finite-five-loop S5 pair,
current four-loop Ready/rolling-S5 comparison, five ordering scouts and the
source-selection qualification complete.
The original frozen release was delivered for a **user-launched trial**, with
honest CPU/work versus wall-time trade-offs; Epoch is not a universal speed
winner. That trial generated all 67 owners but failed in an optional union-reuse
check. Its conservative repair has passed native tests and bounded full-input
validation; the optimized replacement has now built successfully. The user
waived further pre-launch checks and their recovery launcher started a fresh
walk at00:44 October1 Europe/Zurich. The runbook supplies that executable's
identity and fresh-walk commands. Saved generation outputs were reused; deferred
optimized checks are not claimed passed. None of the historical timings below
is silently reassigned to the repaired build.
See the [generation-first release runbook](../five_loop_optimized_generation_runbook.md)
for the final optimized comparisons, exact frozen executable identities, and
the user-selected32-core/600GB launch. The measurements below retain their
original executable identities and do not silently become measurements of that
new configuration.
See [the active plan](../../ASTER_FINAL_PUSH_FOR_ALL_OPTIMIZATION.md) and
[progress log](../../CODEX_PROGRESS.md). Production LC2 was not modified.

Current decision policy, updated after the user's17:29UTC clarification:
the former mandatory1.5x gate is relaxed. Historical measurements and their
then-applicable thresholds are preserved below. A smaller reproducible useful
gain is acceptable, but a same-build Ready comparison is still needed; the new
lookahead result is an Epoch-only ablation, not evidence that Epoch now wins
that comparison. Four-loop non-regression and independently checked coverage
remain requirements.

## Post-repair optimized controls — 30 September, 22:46–22:54 UTC

The deferred ABBA check now uses the actual repaired production executable,
SHA256 `23d836d7b05fac6b007cc1adea84f04548d5bf0e0a94a28ca877f98942da8045`
(source `d55cfb1c`, opt3/fat-LTO/one codegen unit). All four arms use identical
selected-A1 saved payloads: 16 owners, 508 routes, 58 required queries and
32 roots; W16, CPU32–47. Generation and compilation are not included. Production
was already running and was neither gated nor modified by these checks.

| Arm | Native through drain, s | Independent cold-All, s | Primary total, s | Native CPU, s | Domains | Native inspections |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Ready 1 | 6.354 | 8.140 | 14.494 | 30.136 | 38,923 | 15,269 |
| Epoch 1 | 6.754 | 9.136 | 15.890 | 22.965 | 26,025 | 17,957 |
| Epoch 2 | 7.157 | 8.140 | 15.297 | 23.216 | 26,025 | 17,957 |
| Ready 2 | 5.782 | 8.138 | 13.920 | 28.542 | 38,818 | 15,330 |

Every arm passes native cold-All for all 58 queries and 32 roots, re-inspecting
every recorded native with zero violations, frontiers or uncovered obligations.
Independent audit confirms the receipts and interpretation; all twelve owned
process groups drained. **Epoch is 9.63% and 9.89% slower in
the paired primary totals**, so four-loop wall-time non-regression remains
unpassed. It uses 19–24% less native CPU and retains about 33% fewer domains,
but makes 17–18% more native inspections. Fewer domains does not mean less
inspection work. Short arms produced no foreign-CPU samples; the two production
campaigns ran on disjoint reservations, but other host effects are not excluded.

Epoch's checkpoint-only native exit4 and secondary Python INCOMPLETE are retained
and distinguished from its actual native cold-All PASS. Do not add the unequal
secondary diagnostic workloads to manufacture a speed win. The small control
exercises neither new union fallback; the prior full-input repair regression
does. Evidence, exact commands, memory measures and acceptance receipts:
`TMP/postlaunch-20260930/optimized-repair-four-qualification/RESULTS.{md,json}`.
These measurements supersede the earlier pending repair controls, not the
provenance of any historical timing below.

The first two steady read-only intervals of the repaired full campaign attribute
28.62% and 25.87% of coordinator wall to P2, at approximately 4.29 and 4.07
observed cores. This justifies one separately preregistered h0/h2 preparation
screen on CPU0–31 with the same executable and all183 inputs. It is not a
result or a change to production: the arms' evolving graph prefixes can differ,
so equal-duration throughput must not be represented as fixed-work speedup.

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

## Fixed-work finite-five-loop comparison

The first larger pair also passes independent cold-All and exact durable
mathematical-state comparison. Both use the same 67 saved owners/8,246 routes,
W16/B16/FIFO/snapshot/G2 Union, CPU32–47. The actual requested scope is one
bounded R2/A11/D>=9 query containing 1,324 integer starting points, **not** the
116-query production request. Both retain 910,957 domains, 743,502 native
inspections, 6,861,296 dependency edges and 46,470 cuts, with no frontier or
unresolved obligation. Every native inspection is repeated in a fresh process.

| Arm | Native s | Cold s | Native+cold s | Preparation s | Traversal s | Peak tree RSS GB |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Old h0 | 323.692 | 132.199 | 455.891 | 81.310 | 236.274 | 6.759 |
| New h0 | 326.514 | 138.199 | 464.713 | 80.207 | 240.403 | 6.564 |

This is one pair, approximately 1.94% slower overall, not a speedup claim.
Both arms are uncensored; the documented checkpoint-only exit4 is followed by
successful independent native cold-All acceptance. Native output finalization
is explicitly not evaluated by this CP6 transport; no invented finalization
timing is subtracted. Cold owner preparation alone takes roughly82 seconds.
Sampled foreign CPU load is0.203/0.214 cores. Cached recursive-closure counts
differ because maintenance is time-driven, but both cold checks find every
domain closed in the same graph.

P3 decreases11.911→5.003s, offset by P2 increasing13.273→15.369s, inspection
181.966→190.349s and boundary10.083→10.635s. Summed inspector lookup time rises
32.548→45.827s; this is overlapping per-inspection time, not coordinator wall.
The new P2 breakdown is source resolution10.693s, canonical deduplication1.383s,
antichain0.304s, representative selection0.172s, reverse lookup2.024s and
transfer0.316s. Assigning many more preparation helpers cannot fix the dominant
inspection phase by itself.

The new checkpoint totals about0.500GB versus1.143GB old (56.2% smaller),
with records0.360GB versus1.003GB. However, native cold loading takes12.972s
versus5.578s, so the compact representation does not yet imply faster startup.

A single read-only pass after both timed runs groups the743,502 timed records
by their46,470 persisted merge epochs (the167,455 aliases carry no native
timer). Callback wall times sum to314.570577s; per-cut maxima sum to180.013936s,
almost the181.966252s observed inspection wall. Even the ideal per-cut bound
`max(longest_callback, sum_callbacks / 15)` totals180.013944s. This supports
straggler-limited fixed batches rather than predominantly unrelated waiting.
All ten longest records are G2 Apply residuals of owner `000011001001011`,
maximum2.134845s. These timers are not CPU time and do not isolate guard
planning from algebra. The independently reviewed aggregation is retained in
`s5-pilot-plan/FIVE_FINITE_RECORD_SKEW.json`.

The next discriminating experiment is existing rolling oldest-prefix
publication (window76, cut16), compared with a fresh current Ready baseline.
Do not equate inspection-phase wall time with pure barrier waste: the read-only
aggregation above supports testing overlap across straggler-limited cuts.
Cold reinspection has an independent, already-known work list and can exploit
parallelism differently; its speed is not a direct scheduling benchmark.
Evidence: `s5-pilot-plan/runs/{old,new}-h0-r1/five-finite/` and
`s5-pilot-plan/five-finite-old-new-r1-state.json`.

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

The existing `owner-guarded-apply` diagnostic subsequently exposed the actual
guards without a rebuild. Rule11 excludes both `n3 - 1 = 0` and
`1 - n2 + n1 = 0`; rule13 has equality `1 - n2 + n1 = 0`, excludes `n2 - 2 = 0`,
and is explicitly an affine case. These are native **zero-based** physical
index symbols, not the nonnegative owner offsets. Thus a generated rule exists
on the problematic diagonal `n2 = n1 + 1`; the rectangular walk cannot classify
that mixed face uniformly. The diagnostics retained original RHS denominators
but stopped at their 64-event display cap (exit4), so they are not complete
one-hop or closure proofs. Both small CPU16 diagnostics drained in 4.290 seconds
combined. B2 stays rejected as a campaign candidate; exact affine-domain support
or an alternative order avoiding this guard would be a reopening condition,
not a reason to narrow the required queries.
Guard-display receipts are in `order-pilot-plans/diagnostics/b2-guards/`;
the consolidated scout report is `order-pilot-plans/SCOUT_RESULTS_20260930.md`.

Evidence: `order-pilot-plans/prepared-v5/` below the same local evidence base,
including frozen descriptors, native commands, complete generation checkpoints,
staging receipts, admitted identities, saved walks and independent cold reports.
There is no production restart recommendation from this unpaired portfolio.

## Reproduced source-selection qualification

Two subsequent fresh matched pairs use the same frozen binaries, CPU64–79/W16,
complete root downsets and all58 required queries, with execution order A0→A1
then A1→A0. Every arm regenerates its own programs, passes native owner/order
admission and independently cold-verifies all required queries. The primary
phase sum is unchanged from the scouts; neither previous artifacts nor previous
timings are recycled into these pairs.

| Pair | A0 total s | A1 total s | Time reduction | A0 domains | A1 domains | Domain reduction |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 1, A0 then A1 | 97.286 | 84.538 | 13.10% | 69,315 | 38,680 | 44.20% |
| 2, A1 then A0 | 102.024 | 82.553 | 19.08% | 64,231 | 38,681 | 39.78% |

This passes the predeclared **at least20% work-reduction criterion in both
pairs, without an offsetting throughput regression**. It does not pass the
20% timing-only criterion in both pairs. Native inspections fall37.27%/37.32%,
waited-child CPU27.37%/27.84%, and maximum single-child RSS15.51%/12.11%.
The latter is not aggregate peak RSS. Ready's timing-dependent publication can
change graph counts slightly; physical query coverage remains identical.

The short walks produced no10-second foreign-CPU samples, so absence of samples
means unknown contention, not an idle host. An unrelated compiler was observed
with affinity0–383; it was not modified. Sampled pending counts are lower bounds,
not exact queue peaks. All four runs were uncensored, all24 owned groups drained,
and an independent agent checked actual commands, identities, receipts and gate
interpretation. Evidence: `order-pilot-plans/QUALIFICATION_RESULTS.{md,json}`
and `qualification-pair{1,2}/`.

The qualified intervention is **finite source visitation**, not a changed
mathematical integral comparator. The subsequent small five-loop transfers below
are positive; the structural B1 ablation is completed and parked. Full
production-policy qualification, Epoch's stronger1.5× gate and production
closure are still open.

### Limited five-loop source-selection transfer

One fresh A0→A1 pair uses the same frozen executables, W16/CPU64–79 and a
784-point query (R<=1, A<=10, D>=9) over14 literal owners/routes. Both generate
all14 sectors without reuse, admit the saved programs and pass cold-All for the
same query with no uncovered obligations or frontiers. This is **not** the
67-owner/116-required-query production request. Both arms use depth2, unrestricted
generation rank, SearchFinite and sparse exact arithmetic—not LC2's depth0/R10/
sparse-factorized generation policy.

| Quantity | A0 default | A1 source-selected |
| --- | ---: | ---: |
| Fresh generation s | 21.157 | 15.162 |
| Staging + native admission s | 2.310 | 2.300 |
| Guarded walk wrapper s | 2.621 | 1.539 |
| Cold-All s | 2.152 | 1.149 |
| Charged phase sum s | 28.239 | 20.149 |
| Domains / native inspections | 3,149 / 2,840 | 2,185 / 1,947 |
| Generated rules / finite residuals | 4,283 / 14 | 698 / 14 |
| Bundle MB | 10.200 | 3.254 |
| All-phase waited CPU s | 149.939 | 75.029 |
| Maximum single-child RSS MB | 386.91 | 302.13 |

A1 reduces measured phase time28.65% and domain work30.61%, with unchanged
finite residual count. This is one positive transfer pair, not independent
qualification of the production policy or a minimal-master claim. Whole-arm
clocks155.791/103.695s include manual interphase reviews and remain below1800s;
those gaps are not solver speed. Short native walks provide no foreign-load
samples, so contention is unknown. All10 owned groups drained. An independent
agent audited input hashes, actual commands, receipts and native cold results.
Evidence: `order-pilot-plans/FIVE_TRANSFER_RESULTS.{md,json}` and
`five-transfer-current/`.

### Separate transfer with LC2's generation policy

A second fresh pair keeps the same14-owner/784-point input and switches both
arms to the actual production generation policy: depth0, R10, SearchFinite,
sparse-factorized. Only A1's source visitation differs between arms. Explicit
case resources match (8,192 work items,100,000 terms,1,024 normalizations,
4,096 factorizations); these are work limits, not truncated coverage. The
source-order strategy, optimized binaries, CPU placement and W16 remain fixed.

| Quantity | A0 default | A1 source-selected |
| --- | ---: | ---: |
| Fresh generation s | 20.157 | 14.157 |
| Staging + native admission s | 2.303 | 2.303 |
| Guarded walk wrapper s | 2.619 | 2.625 |
| Cold-All s | 2.154 | 1.150 |
| Charged phase sum s | 27.232 | 20.235 |
| Domains / native inspections | 3,151 / 2,843 | 2,191 / 1,947 |
| Generated rules / finite residuals | 4,279 / 18 | 696 / 16 |
| Bundle MB | 10.173 | 3.245 |
| All-phase waited CPU s | 129.661 | 52.900 |
| Maximum single-child RSS MB | 243.86 | 130.24 |

Both fresh generations and native admissions pass; cold-All independently
reinspects every reached native obligation and the unchanged sole query, with
zero uncovered cases, errors or frontiers. A1 lowers charged time25.70% and
domain work30.47%. Walk time alone is neutral: generation and cold verification
provide the elapsed-time improvement. The finite residual count18→16 genuinely
differs; neither minimality nor equality of terminal bases is claimed.

This is one production-policy **transfer** pair, not a full67-owner solve or
two-pair production qualification. Both whole-arm deadlines are under94s and
all10 process groups drained. Short runs again lack periodic contention
samples; other users' builds were observed nearby and left untouched. Actual
argv, source/policy bindings and cold receipts passed independent audit.
Evidence: `order-pilot-plans/FIVE_PRODUCTION_POLICY_RESULTS.{md,json}` and
`five-transfer-production-policy/`.

### B1 degree-row ablation: no useful gain

One further input-only four-loop experiment keeps B1's shared-support sector
weights/priorities but removes its private-excess degree row, retaining the
ordinary E-then-R degree order. Fresh314+328-sector generation,16 selected
owners/508 routes and all58 required queries pass native admission and cold-All:
38,819 domains,15,332 native inspections,471,368 edges and zero uncovered
obligations or frontiers. All6 owned process groups drained.

The charged phase sum is91.955s (generation40.180+34.169s, walking6.137s,
cold-All9.155s plus staging/admission), compared with prior strong-A1
qualification84.538/82.553s and about38.68k domains/15.3k inspections. This is
one ablation, not a new paired qualification. Removing the degree row does not
reveal a useful sector-priority gain; park this candidate instead of extending
it into a blind permutation sweep. It does not rule out other lawful runtime
comparators. Frozen executable711b18c5/560f identity and physical query scope
are unchanged; only the requested descriptor differs. Independent receipt
audit passed. Evidence: `order-pilot-plans/b1-sector-only/`.

## Predeclared Ready/Epoch timing and audit boundary

Before running the new Ready comparison, the common primary metric is fixed as
native process launch through owned-group drain **plus independent native
cold-All verification**, for both policies. Actual required-query coverage,
frontiers, errors, dependency closure and full native reinspection must pass.
The secondary Python event audit is timed and reported separately for both
policies. A timeout there is **incomplete, never a pass**; it is not added only
to Ready's primary timing or used to turn an unsuccessful native run into an
accepted comparison. A genuine secondary contradiction blocks interpretation
until resolved. A valid primary timing with an incomplete secondary audit must
not be described as an all-audits-passed deployment.

This clarification is registered before the new runs, because the historical
million-record Ready Python audit exceeded 300 seconds despite successful
native cold-All checking. Native cold verification already checks the full
result/checkpoint binding, dependency graph and every native inspection. No
new proof mechanism or weaker scoped native gate is introduced. Ready's full
JSON/CP5 output and Epoch's summary/CP6 output costs both remain charged to
their respective native boundaries: this is end-to-end verified throughput,
not a pure scheduler benchmark. The 1.5× gate requires two matched pairs.

## Current Ready versus rolling S5: four-loop ABBA

The same optimized binary and original saved16-owner/508-route input were
compared in Ready→prefix76→prefix76→Ready order. Source selection was **not**
changed in one scheduler arm. Both use16 total workers on CPU32–47. Ready's
automatic partition is8 inspectors,7 admission helpers and1 coordinator;
Epoch uses15 inspectors and1 coordinator, FIFO, snapshot lookup, G2 Union and
oldest-prefix publication with window76/cut16. This measures the actual policies
within the same total budget, not equal inspector counts.

| Arm | Native s | Cold-All s | Primary sum s | Native CPU s | Peak native RSS MB | Domains | Native inspections |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Ready1 | 9.779 | 12.150 | 21.929 | 30.695 | 245.6 | 69,156 | 24,406 |
| Prefix1 | 9.583 | 16.153 | 25.736 | 20.834 | 331.4 | 51,143 | 31,895 |
| Prefix2 | 9.578 | 13.152 | 22.730 | 20.856 | 316.4 | 51,143 | 31,895 |
| Ready2 | 8.773 | 14.158 | 22.931 | 31.075 | 243.4 | 66,595 | 24,356 |

All four native/cold comparisons pass the exact frozen scope:58 required
queries,32 roots, every native inspection repeated, zero errors/frontiers or
uncovered obligations. Medians are22.429834s Ready and24.233158s prefix:
**8.04% slower**, with opposite per-pair signs. The1.5× gate fails. Ready's two
secondary Python audits also pass (6.153/5.150s); Epoch keeps the previously
declared CP6 secondary INCOMPLETE transport. All owned groups drained and each
whole arm remained below1800s. Short native runs gave no10-second contention
samples; contention is unknown, not assumed zero.

Rolling is working: inspect/wait drops to3.328/3.384s from fixed-cut roughly8.2s.
Snapshot refresh costs only0.104s per run, with no refill lacking the current
snapshot. However, prefix retains about31% more native inspections despite fewer
domains. Traversal remains5.674/5.726s versus Ready4.994/4.950s. Different work
mix and cold reinspection offset the local barrier relief. Neither fewer domains
nor more available workers establish whole-campaign improvement.

Ready writes64–66MB of full result JSON plus46–47MB of CP5 checkpoint; prefix
writes roughly10KB of summary plus20.239MB of CP6 checkpoint. Both actual
materialization costs remain charged. Prefix cold loading is roughly0.497s,
versus Ready0.35s, despite its smaller files. This is not a format-normalized
scheduler microbenchmark. Raw receipts and complete phase/storage metrics:
`s5-pilot-plan/ready-followthrough/FOUR_ALL_READY_PREFIX76_RESULTS.{md,json}`.

## Current Ready versus rolling S5: finite-five first pair

This returns to the larger saved-rule control:67 owners/8,246 routes, one
1,324-point R<=2/A<=11/D>=9 query, original programs, W16/CPU32–47, no IBP
regeneration. Both arms finish naturally and independently cold-verify their
entire native work and sole required query/root. Neither proves production-wide
or unrestricted five-loop closure.

| Quantity | Ready | Prefix76/cut16 |
| --- | ---: | ---: |
| Native s | 167.841 | 219.549 |
| Native owner preparation s | 81.528 | 79.942 |
| Traversal s | 71.034 | 133.595 |
| Cold-All s | 150.202 | 141.205 |
| Primary native+cold s | 318.043 | 360.754 |
| Native CPU s | 536.576 | 476.732 |
| Peak process-tree RSS GB | 6.233 | 6.568 |
| Interior sampled process-tree cores | 6.330 | 2.908 |
| Domains | 967,843 | 919,534 |
| Native inspections | 760,719 | 749,157 |
| Dependency edges | 7,146,573 | 6,978,690 |

Epoch is13.43% slower in this single pair; no deployment gate passes and no
automatic repeat follows. It performs slightly fewer routed admissions and
Apply successors, using less CPU, but has much more elapsed waiting. Rolling
inspect time improves190.349→80.435s versus fixed-cut Epoch; the contemporaneous
Ready comparison remains the relevant baseline. Prefix waits total80.290s;
P1/P2/P3/boundary cost19.884/17.550/5.020/10.354s. Snapshot refresh costs7.543s
across93,823 calls. These observations do not establish that adding workers
alone will help. Core samples include helpers/coordinator, not just inspectors.

Cold phases (Ready→Epoch) are load8.195→13.634s, owner preparation81.285→83.590s,
graph checking27.779→9.924s and native reinspection28.623→30.619s. Ready writes
1.435GB full JSON plus0.930GB CP5; Epoch writes a10.6KB summary plus0.506GB CP6.
All costs remain charged. Fixed loading/preparation makes this finite control
a limited scaling discriminator: eliminating only Ready's entire traversal,
with everything else unchanged, would cap overall speedup at1.287×. This is
an explanatory bound, not permission to redefine the comparison after the fact.

Ready's separate Python diagnostic times out after241.233s (exit124) in
`union_covered`: **INCOMPLETE, not PASS**. Its primary native cold-All and full
result/checkpoint binding pass. Epoch retains its documented CP6 secondary
INCOMPLETE transport. No mathematical contradiction was reported. Whole arms
remain below1800s and every owned group drained. Independent receipt audit
passed; production was untouched. Evidence:
`s5-pilot-plan/ready-followthrough/FIVE_FINITE_READY_PREFIX76_RESULTS.{md,json}`.

A proposed cut1 diagnostic was cancelled before preparation: the preserved
September29 experiment had already failed to reduce the extra Route work
(31,558→31,547), despite passing all58 four-loop queries. Window differences
alone do not justify repeating it. Keep that rejected hypothesis, alongside
the earlier negative oldest-ready and adaptive-dispatch results, when choosing
the next architectural test.

## Next discriminating measurements

The selected-sector generation and observational instrumentation now pass the
native correctness gate:1,215 application tests,32 CLI/API/inspector tests and
23 installed-Python tests. The installed-Python harness first omitted the wheel
path from its child environment; that failed receipt is preserved alongside the
passing rerun. Repository test assertions were unchanged. A separate25-test
stager suite checks metadata, not mathematical admission.

Actual selected-owner integration also passed:4+12 fresh sector jobs (no reused
shards), all16 owners natively admitted, all508 routes retained and the original
58 required queries unchanged. The Ready walk drained39,178domains with
14,878native inspections and472,625edges. Independent cold-All reinspection
accepted all14,878 inspections,58 queries and32 starting roots without uncovered
obligations, frontiers or violations. All six guarded process groups drained.
Their phase times sum to25.704s in the correctness profile; this is **not** a
speed comparison with the optimized full-downset measurements. Evidence:
`order-pilot-plans/selected-runtime-next/four-selected/`.

A subsequent read-only comparison, independently audited, found all16 fresh
owner payloads byte-identical to the full-root A1 outputs (5,128,567bytes total).
Generation/staging used only fresh checkpoints. This is stronger evidence for
these particular files, not a promise of generally byte-stable Symbolica dumps.
The selected path avoided626 unused sector solves without changing the delivered
owner programs in this control.

The new diagnostic executable is frozen at
`859698b6c40da017009cb9e2454a5b008dd49dd595d5d9411e455daed9c42778`
(`selected-profile-bin/rustred` in the current evidence directory). Its app crate
uses opt-level1 with the existing optimized core cache. It is suitable for
correctness checks and causal localization, **not** for qualifying speed against
the fully optimized binaries reported above. Generation checkpoint schema is
now v4; unchanged candidate program payloads still use their existing binary
schema. Old generation checkpoints are rejected rather than migrated.

### Observational Epoch diagnostic

For bounded developer pilots, setting `RUSTRED_EPOCH_PROFILE=1` before invocation
adds `epoch.rolling_diagnostics.wait_profile` and `inspection_profile` to the
final report. This private diagnostic is off by default. It changes no job
protocol, persisted checkpoint authority, scheduling policy or rule semantics.
Cold restoration starts new observations; it does not restore old wall timings.

- Wait classes distinguish publication retention, queued/computing prefix work,
  returned-receipt drainage and full-window credit blockage. The credit-blocked
  classification requires an actual missing computing prefix, returned tails,
  pending work, a full window, no queued jobs and unused inspector capacity.
- Exclusive poll-duration buckets reconcile with existing blocking-poll time.
  Occupancy-weighted values sample each poll's start state; they are wall-time
  estimates, not measured CPU time or proof of useful work forgone.
- The collector retains only32 slow jobs and32 coalesced prefix-blocker windows
  plus the current window. Shared invocation-relative timestamps permit bounded
  correlation. Aggregated worker durations are not a critical-path duration.
- First event and first `Admit` timestamps distinguish late emission from
  possible early emission; an `Admit` may still be redundant. Neither grants
  permission to publish a partial source or discard remaining obligations.
- Off mode introduces no per-event clock or profile lock. On-mode poisoning or
  unavailable activity is explicitly reported, never promoted to authority.

Both fresh execution and restored-checkpoint tests compare mathematical state
and durable graph sections with profiling on/off. Native pilot graph comparison
and independent cold-All reinspection remain required after this unit gate.

### Completed finite-five diagnostic pair

Both arms completed and passed independent cold-All reinspection of all749,157
native cases. Each produced919,534domains and6,978,690edges for the same single
1,324-point query. Both satisfy the existing CP6 acceptance contract, including
its explicit secondary Python transport limitation. There were no uncovered
obligations, errors or frontiers; all six owned process groups drained.

The strict comparison preserves geometry, inspected/sealed flags, ledger,
ordered dependency targets, anchors/pins, owner/query identity, orthants, rescue,
logical records and actual physical lookup counters. Those all match. Cached
periodic closure counts differ, but independent cold reinspection closes all
919,534domains in both arms. This is **not** a claim of byte-identical checkpoints
or complete typed-record file identity: diagnostic timing fields differ.

| Correctness-profile observation | Profiling off | Profiling on |
|---|---:|---:|
| Native through process drain, s | 275.021 | 272.214 |
| Cold-All, s | 159.202 | 159.209 |
| Traversal, s | 184.027 | 182.654 |
| Blocking polls, s | 98.290 | 97.712 |
| Native inspections | 749,157 | 749,157 |

This pair is diagnostic, not optimized performance qualification. Its measured
mechanism is nevertheless specific:95.841s of the enabled arm's97.712s blocking
polls (98.09%) met the exact start-state credit-blocked condition. Wait-sampled
means were2.631 computing workers and72.816 returned results, against15 inspector
slots and a76-job logical window. These are occupancy estimates, not CPU usage.

The retained32 blocker windows account for17.538s. Nineteen match retained slow
jobs, accounting for13.560s; all are Apply jobs,18 entirely after the first event
and16 entirely after the first Admit. The rest straddle those boundaries or have
no retained matching job. Early emission is therefore demonstrated for that
subset, not for all97.7s. First Admit is not necessarily useful new work.

This supports reviewing one bounded whole-result lookahead experiment. Merely
recycling physical worker slots cannot help while all76 logical reservations
remain occupied: the proposal must explicitly allow additional bounded logical
lookahead or safely remove reservations. Preserve oldest-prefix publication and
whole-job authority, cap both total reservations and retained result bytes, and
measure redundant native work and stale-snapshot effects. Do not present this
as free concurrency, a proven speedup or authorization for partial-source CP6
publication. Evidence and strict comparator receipt are in
`s5-pilot-plan/ready-followthrough/profile-diagnostics/`.

### Bounded lookahead: implementation and first campaign screen

The opt-in implementation is now tested. It separates returned whole-result
storage from physical inspector slots, and permits an explicit extra logical
inventory beyond the base window. Default extra capacity is zero. Oldest-prefix
publication, exact merge gates and persistence of every unresolved reservation
are unchanged. A returned-buffer byte allowance controls extra admission; it is
not a hard RSS limit because in-flight jobs can return after admission stops.
Rust, CLI and Python expose the same controls. Native scalar schema5 carries
the explicit base window, extra allowance and byte policy.

Native acceptance:1,229 library tests passed,12 existing fixture/benchmark tests
intentionally ignored, zero failures; nine public-interface tests also passed.
An initial malformed synthetic lookup fixture was corrected without changing
its quarantine assertions or the engine. The failing receipt is retained.
The rebuilt library suite, source review and process drain were independently
audited before any campaign was launched.

The first campaign pair uses the same16 selected A1 owners,508 routes and58
required queries, with base76/cut16,16 workers on CPUs32–47, FIFO/snapshot/G2
Union and no preparation helpers. Both arms use the same frozen app-opt1 CLI,
SHA256`d68ada4577ccd71489375d536134a30a0a6b3c0ed10d1f3ec6fb98ffc56c319b`.
This is a causal/correctness screen, not the fully optimized deployment gate.

| Four-loop screen | Extra0 | Extra32 |
|---|---:|---:|
| Native through process drain, s |9.372|8.562|
| Core traversal, s |5.187941|5.210677|
| Cold-All guard, s |11.150524|11.144431|
| Secondary diagnostic guard, s |1.141054|1.137377|
| Native inspections |16,932|16,964|
| Domains |25,945|25,948|
| Dependency edges |481,213|481,694|

Both independently cold-verify all58 required queries and32 roots, with no
uncovered obligations or frontiers. Extra32 dispatched448 additional-lookahead
jobs, reached108/108 logical reservations and20.895MB retained result capacity,
and reported no overshoot of its256MiB admission allowance. Inspections grew
0.19%; no reservations or returned buffers remained after drain. Flag-off E0
also matches the prior accepted checkpoint's durable mathematical state and
physical lookup/verification counters exactly. Typed-record byte identity is
not claimed across versions.

Traversal and blocking time were essentially unchanged. The lower
launcher-inclusive phase sum is not evidence of a causal engine speedup in
this single short pair; the periodic contention sampler did not obtain a
sample before either native run ended. The result clears the integration and
safety gate, not the performance gate. Evidence:
`order-pilot-plans/escrow-next/FOUR_RESULTS.{md,json}` and its per-arm receipts.

The larger registered finite-five E0/E1024 pair has also completed and passed
independent cold-All verification. It retains one1,324-point query,67 saved
owners and8,246 routes; it does not cover the116-query production scope. Each
arm finished within its own inclusive30-minute allowance. The finite lookahead
was selected from measured slow-job sequence gaps, not a blind capacity sweep.

| Finite-five screen | Extra0 | Extra1024 |
|---|---:|---:|
| Native through process drain, s |274.294|214.555|
| Owner preparation, s |84.942|84.873|
| Core traversal, s |182.660534|123.744910|
| Cold-All guard, s |156.206282|157.196070|
| Secondary diagnostic guard, s |1.141144|1.137591|
| Native + cold-All, s |430.500282|371.751070|
| All charged phases, s |431.641426|372.888661|
| Native CPU, s |631.035|632.496|
| Native inspections |749,157|754,706|
| Domains |919,534|925,732|
| Dependency edges |6,978,690|7,090,301|
| Blocking polls, s |97.835|37.936|
| Sampled own cores, whole invocation |2.307|2.967|

Both cold verifiers replay every native inspection, close the required query
and find zero uncovered obligations, frontiers or violations. The new E0
matches the historical accepted E0 durable mathematical state and physical
lookup/verification counters. Extra1024 need not have an identical graph:
lookahead changes dispatch snapshots and selected work, which is independently
checked rather than normalized away.

The result supports the proposed mechanism: **32.25% less traversal time,
21.78% less native wall time and13.61% less time across all charged phases**.
Waiting falls by59.90s, accounting for almost all59.74s of native wall reduction.
Native CPU grows0.23%, inspections0.74%, domains0.67% and edges1.60%. There is
no large redundant-work penalty in this control. Sampled foreign load averaged
0.203 versus0.281 cores; this is still one pair, not a repeated qualification.

Extra1024 issued525,512 cumulative extra-lookahead dispatches; these recycle
credits and are **not** that many additional total inspections. Peak logical
inventory was1100/1100, peak retained buffer capacity32,614,280B against256MiB,
with no reported overshoot and no remaining reservations after drain. Sampled
whole-invocation activity includes about85s of predominantly serial startup;
2.967 cores is not a claim of16-core or20-core saturation. All six owned
native/cold/secondary process groups drained, with locks released before any
next heavy job. Evidence is under`order-pilot-plans/escrow-next/`.

This is a meaningful causal result, but **not** the fully optimized1.5x
verified-throughput gate. Do not compare these app-opt1 timings directly to the
older campaign-profile Ready measurements. The next performance step must use
one frozen fully optimized binary and a contemporaneous Ready baseline.

For perspective, removing all95.841s of the earlier diagnostic wait while
holding all other costs fixed could improve its431.423s native+cold boundary
by at most1.286x (22.2% less wall time), versus2.104x for traversal alone.
This is an optimistic arithmetic bound for that isolated component, not a
prediction or a bound on changes that also reduce preparation or total work.
The1.5x deployment requirement still needs a stronger measured result.

### Next bounded steps

1. Both limited five-loop source transfers are positive and audited; prepare
   full67-owner integration without claiming it complete. B1's degree-row
   ablation is now completed and parked for lack of gain. B2 stays parked
   following its concrete coupled-affine diagnosis. Preserve failed candidates
   rather than repeatedly trying blind permutations.
2. The finite-five lookahead pair now passes cold coverage and its causal
   work-inflation screen. Freeze an audited optimized milestone for matched
   qualification, including a contemporaneous Ready baseline and the prepared
   traversal-heavy control. No partial-source CP6 publication or blind window
   sweep follows from it. The existing optimized Ready comparison remains
   negative until new matched evidence replaces it.
3. Selected-sector Rust/CLI/Python, Ready and Epoch four-loop integration pass.
   The bounded standalone3822 A1 generation probe was censored without a
   completed shard; full67-owner generation affordability remains unknown.
   Production preparation must retain all116 required queries and67 auxiliary
   helpers. Missing sectors remain uncovered, not implicit terminals. Do not
   replace a missing regenerated owner with an old program and call it fresh.

The independent opt-level-1 finite-five correctness pair already verifies one
bounded query over 910,957 domains and 743,502 native inspections with identical
helper0/helper2 graphs. Those runs overlapped compilation and cannot supply
production timing evidence or a claim that all five-loop queries close.
