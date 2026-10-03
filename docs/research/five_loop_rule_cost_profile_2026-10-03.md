# Bounded five-loop campaign cost profile

October 3, 2026. This is discovery evidence for the
[profile-guided optimization plan](../../PROFILE_GUIDED_RULE_OPTIMIZATION_PLAN.md),
not a campaign census, candidate performance result or closure claim.

## Method and identities

The historical stopped Oct1 and Oct2 campaigns use CP6/schema-1 framed binary
record sidecars. The older `records_scan.py`/`records_cost.py` helpers expect
CP5 JSONL; `records_census.py` also has a historical scratchpad dependency.
The new generic research reader
[`profile_records.py`](../../tools/research/rule_optimizer/profile_records.py)
follows the native `epoch/records/{wire,typed}.rs` schema. It reads bounded
windows and never restores a checkpoint or authenticates an entire sidecar.

The sampling plan was frozen at 10:05:27 UTC before reading record bodies.
It selected Oct1 generations 1/13/26/38 and Oct2 generations 1/9/17/25, four
1 MiB windows per segment at byte fractions 1/8,3/8,5/8,7/8. The exact total
was 33,554,432 bytes. Three consecutively decoded frames establish each first
boundary. Record and diagnostic lengths, integer encodings, schema, geometry,
scope and inventories are checked; unknown/corrupt data fails closed. The
executor rejects overlapping windows, changed files and requests above 32 MiB.
The independent audit reran all six focused tests successfully.

The sample decoded 74,384 frames: 47,204 whole native inspections, 11,937 G2
residual inspections and 15,243 delegated rows. Boundary omissions total
14,469 prefix bytes and 8,840 incomplete-tail bytes. Window/frame offsets and
SHA256 receipts, 200 slowest rows and 9,324 stratum representatives are saved.
No live record bodies were read; current campaigns supplied metadata only.

Stopped Oct1 has 38 sealed segments totaling 127,541,069,929 bytes and
260,112,277 records. Stopped Oct2 has 25 totaling 80,942,143,373 bytes and
177,522,047 records. These counts come from compact committed inventories.
Both bind query SHA256 `42a0c62771b6e7c53cc937d46ad9505e33c282db31a8d6f64846ecbca749ef64`.
Oct1 CLI `4e76707b…` differs from Oct2/current `8ef80b52…`; comparisons of
their inspector times cannot isolate a rule-pool effect.

## Complete-run metadata and sampled mechanisms

Oct1 stopped with a frontier after 222,194,016 native inspections and
5,050,453,156 committed events. Its 2,737,075,967 dependency edges and
36,847,690 recorded closed domains are different counters. The result's
closure snapshot was 8,941.76 s old, a conservative lower bound, with 13/67
closed initial obligations. It does not establish 116 required queries closed.

Oct1 invocation-local inspector lookup records 4,151,342,058 queries,
3,816,788,495 stored hits and 288,338.97 summed lookup seconds. Coordinator
phase wall includes P2 55,223.61 s, P3 40,808.92 s and inspect 15,224.23 s;
preparation reverse accounting is 47,418.54 s. These scopes differ and overlap.
They motivate measuring sharing/resolution cost, without assigning it to rules
or summing the fields into end-to-end CPU/wall time.

| Stopped sample | Apply inspections / inspector seconds | Route inspections / inspector seconds |
| --- | ---: | ---: |
| Oct1 | 10,798 / 71.3973 | 17,939 / 8.03735 |
| Oct2 | 10,013 / 49.74465 | 20,391 / 11.83722 |

The first ranked hypotheses concern two Apply mechanisms:

1. Owner `101010000110001`: 11 sampled inspections / 6.12255 s. Eight whole
   domains at R<=16, A<=14, -2<=D<=14 contribute 6.07654 s, 9,425 selected
   pieces, 211,504 successor events and 975 distinct outgoing graph edges.
   Node189583247 alone has 1,736 pieces, 33,380 successors, 139 edges and
   0.948533 s. Test exact source combinations that simplify whole-piece
   application or eliminate repeated successor blocks. A shorter RHS alone
   does not pass: completed downstream union work/cost must improve.
2. Owner `111100000011100`: 368 sampled inspections / 8.55857 s, 6,981 selected
   pieces, 114,527 successor events and 12,246 outgoing edges. Whole node
   189583174 at R<=16/A<=16/D=0 has 155 pieces, 2,905 successors and 276 edges.
   Node189583489 has identical phase/owner/scope/A/R/D caps but different exact
   coordinate bounds: 26 pieces, 380 successors and 32 edges. Preserve both
   regions to detect candidates that merely fragment or transfer obligations.

A distinct frequent pattern is low rank with high positive powers. Owner
`110010101101011` contributes 1,029 sampled inspections / 11.1516 s, 16,189
successors and 14,877 edges. Repeated strata have R<=1, A<=21..23, D around
20..23. Bounds describe symbolic domains, not attained physical tuples.

Routing remains part of every downstream comparison. Oct1 sampled Route work
emits 226,358 Route domains and 17,356 Apply domains, pruning 1,050,943 of
1,295,240 masks. Control node6613266 (`110110111001001`) emits one Apply and
122 Route domains, with 259/382 masks pruned. No causal parent/child connection
is inferred from proximity to an Apply record.

## Limitations and frozen panel

Oct1 generation26/window11 contributes 22.6307 of 71.3973 sampled Apply seconds
and 451,124 of 839,942 sampled successors. Both training owners occur in this
retained cluster. Purposeful generations, fixed byte windows, variable record
sizes, truncated boundaries and publication/stage dependence preclude a
population frequency estimate. Oct2's short final segment receives the same
sample bytes as larger segments. These are mechanism nominations, not a global
ranking of rule costs or evidence that any candidate improves campaign speed.

CP6 records lack selected-rule identity, newly admitted child counts and
required/helper root ancestry. Accepted events are not new children.
`distinct_edges` includes existing-node obligations and G2 anchor edges.
`known_reuse` is earlier same-job emission reuse, separate from snapshot lookup
hits. Neither sharing nor local inspection certifies recursive closure.
Matching `rules` is a work counter, not a selected rule identifier.

The apparently cheap same-owner neighbor of node189583247 is G2 node189583202
with an empty residual and seven anchor edges. It is explicitly excluded from
matched performance comparison. Its zero-query diagnostic must not be launched.

Before candidate selection, the panel policy reserved training families
`101010000110001`/`111100000011100`, held-out families
`001100101110000`/`110010101101011`, and Route control `110110111001001`.
Historical costs were observed; held-out means excluded from candidate fitting.
The first holdout shares discovery window11 and tests owner transfer, not an
independent time cluster. The other uses final window15; Route control uses
early window1. All exact geometry, source owner indices/payload identities and
same-owner original required/helper rows are retained; ancestry remains unknown.

The first prepared native diagnostic uses `owner-domain-match` without
`--follow-successors`, only three training/neighbor queries, and the current
frozen repaired old saved pool (67 owners, 8,246 routes, two overlays).
This emits actual selected batch/rule IDs per exact piece and matching stats,
but no RHS applications, immediate successors or recursive closure. Its CLI
and added repair differ from the historical sample; new matching is a mechanism
inspection, not a historical timing A/B test.

Root completed that inspection in 142.815 s (143.485 s inclusive), peak process
tree RSS 5.101 GB, all groups drained. Preparation took 138.577 s and matching
0.372308 s. All three queries classify completely: 1,917 selected pieces plus
25 terminals, zero gaps/unresolved cases, no summary truncation. Every matching
counter equals the historical record for each exact domain. The first parent
has 1,761 pieces selecting 143 distinct batch0 rules; rule19 appears in 111,
rule0 in 76, rule6 in 74. The second has 155 pieces/49 rules, led by rule40
(18 pieces); its neighbor has 26 pieces/five rules, led by rule25 (12).
This establishes substantial splitting before RHS application, without
attributing RHS work or time per rule. `profiles/native-match-summary.json`
preserves exact examples for the eight most frequent rules per query, original
piece indices, and fixed/free local bounds on zero-based axes 5/6/7/13. The
first rule19 example is piece6 (rank0); piece7 retains rank cap10 with axis13
in [1,10]. Rule40's first example is piece70, with axis5 in [1,9] and axis13
in [0,8]. These caps do not certify attainment or blanket factorization.

Root then inspected batch0/rule19 on five singleton members of piece7:
physical D15 power b=2 with numerator D14 power -s for s=1,2,3,9,
and b=3,s=10. The exact chart also imposes s<=b+7; b=2,s=10 was excluded.
All five applications finished cleanly with seven distinct immediate keys:
six same-owner and one strict-subsector successor, no conditional successors,
RHS problems, optional refusals or coalescing additions. The mandatory
`IncomingComplement` event is not a nonempty frontier witness. All original
poles remain attached; native zero-based `n14` means D15, giving b-1 and
2(b-1), both nonzero here. At b=2,s=2, four lower shifts move the D15 dot
to D3/D5/D10/D11 with coefficient -1 and unchanged A=7,R=2,D=5.
These nominate a structural dot-redistribution target, not a measured costly
tail or budget increase. `profiles/rule19-guarded-summary.json` preserves
all exact singleton geometries, shifts, display-only coefficients and guards.
The single unchanged owner subset supports local algebra only, not cohort cost.

## Next bounded shared-cost design

No frozen five-loop panel case has an established recursive closure receipt.
The first completion-seeking training cohort is the exact 19-point piece7
chart, as one correlated whole query under all 67 owners, 8,246 routes, both
repairs and unchanged terminals. It is a derived training microcohort, not the
83,391,783-point historical parent or a heldout. The other frozen whole training
domains contain 10,296,493 and 3,209,481 integer tuples. The smallest frozen
heldout is the existing one-point A23/R1 case; Route control has 2,926 points,
and the broad heldout has 69,326,952. These exact box/rank/A/D counts are not
runtime estimates: native domains remain symbolic and downstream work unknown.

`profiles/completed-cohort-plan.json` freezes the proposed staged comparisons
and a 1,800-second inclusive pair envelope, subject to root allocation. Both
fresh arms must finish with no unresolved debt and pass bound cold verification
before completed union work/cost ratios are reported. Otherwise retain explicit
censoring, stopping reasons and debt; a separately labelled smaller training
probe may follow but cannot replace the frozen whole cases. Holdouts remain
unused for fitting; full controls and the original 116+67 scope remain required.

The proposed broader candidate allows all active powers positive, only D14
negative, and the eight other inactive powers zero. This is a scope expansion
beyond piece7, requiring its own exact proof/guards/descent. In the primary
parent it intersects 77/1,761 matched pieces; 39 are conservatively wholly
contained by explicit bounds, covering 596 tuples. All intersections total
2,008 tuples; only one of rule19's 111 pieces intersects (piece7). These are
geometry counts, not cost shares or certified applicability. Whole-piece
alternate dispatch cannot use a mere partial intersection. Exact indices and
cap-aware calculations are in `profiles/broad-scope-overlap.json`.

The existing public candidate inspector exposes retained source counts, not
incumbent source weights. Saved seed basis ordinals refer to a preconditioned
basis, not original ordinary rows; original-source recovery remains private.
Public finite replay returns identity/count receipts, not that combination.
Do not reconstruct authority from display RHS text. No export bridge is added;
a future larger ordinary source bank needs a distinct preregistered mechanism.

## Local receipts

Ignored evidence is under `TMP/rule-optimizer-20261003/profiles/`:
`preregistered-window-plan.json` (SHA256 `b2953d39…`),
`bounded-sample.json` (`fa7639ff…`), `panel-policy.json` (`e6b61585…`),
`panel-manifest.json` (`2522eba7…`), native-shaped `panel/*-queries.json`,
`discovery-match-{queries,command,bound}.json`, and the detailed
`HOTSPOT_REPORT.md`. No campaign outputs or rule payloads belong in commits.
The reader's [README](../../tools/research/rule_optimizer/README.md) provides
reproduction commands and field semantics. Production lifecycle remained
untouched throughout profiling.
