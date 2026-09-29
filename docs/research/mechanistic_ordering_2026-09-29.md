# Mechanistic ordering inputs, 2026-09-29

Status: input design and source audit only. No new native job, performance
claim, production mutation, graph canonizer, or algebra implementation. The
prepared inputs are in `TMP/codex-mechanistic-ordering.5NFxVw/`; their exact
permutations and preservation checks are in `input-mapping.json`.
The query source is the actual matched matrix input
`TMP/codex-rolling-measurements.eBFGjR/queries/four-all.json`, including its
explicit top-level `query_roles`. Every variant preserves that complete
role declaration and stable query IDs; no role is inferred from an ID string.

## What is shared

The existing common four-loop A4 basis has ten coordinate slots, but each
maximal physical root has nine lines. The roots are prism `1111111110` and
K3,3 `0111111111`. Their literal intersection is BMW `0111111110`: removing
old slot 0 from prism or old slot 9 from K3,3 gives the same labelled support.
The native-verified routing census makes this more than a two-edge observation:

| Existing four-loop classification | Count |
| --- | ---: |
| Selected owners / complete query rows | 16 / 58 |
| Labelled nonzero source supports / nonidentity routes | 508 / 492 |
| Labelled supports belonging to both parent downsets | 134 |
| Owner classes represented below both parents | 11 |
| Prism single pinches routing to BMW / FG | 3 / 6 |
| K3,3 single pinches routing to BMW | 9 |

The prism BMW-equivalent pinch slots are `[0,3,4]`; the FG-equivalent slots
are `[1,2,5,6,7,8]`. All active K3,3 slots `[1,2,3,4,5,6,7,8,9]` route to BMW
after one pinch. BMW's eight single pinches split four each between owners
`0111111100` and `0111111001`. These are **pinch-equivalence groups**, obtained
from the existing selected route classes, not newly proved automorphism edge
orbits or bridge classifications.

There are two different sharing opportunities. BMW is a shared high-level
interface, whereas the four-line owner `0111100000` has a representative in
the strict labelled downset of every other selected owner. The five-line
owners `0111110000` and `0111100100` each occur below 11 other owners. These
are potential reuse relations, not measured native dependencies: a route
class being available does not imply a particular query reaches it.

The frozen five-loop input has 67 owners, 183 queries (116 required and 67
auxiliary), 8,246 labelled routes, and 8,179 nonidentity routes. Of its classes,
33 occur below all four maximal parents, six below three, 15 below two, and
13 below only one. Of its labelled supports, only 18 occur below all four;
6,436 occur below one parent, 1,484 below two, and 308 below three. Class reuse
is therefore much broader than literal-label overlap suggests.

Some useful five-loop structural priorities are:

| Owner | Lines | Other owner downsets containing its class | Saved bytes |
| --- | ---: | ---: | ---: |
| `000010011001001` | 5 | 66 | 450,353 |
| `001100101110000` | 6 | 60 | 520,842 |
| `010100101110000` | 6 | 57 | 220,164 |
| `111100000011100` | 7 | 38 | 788,160 |

These are plausible cheap shared descendants, not necessarily the most
expensive blockers. Conversely, the shared intersection owner
`011101110111000` has an approximately 817 MB saved program, and the low-line
owner `000011001001011` is approximately 102 MB. Pure line-count or fan-in
ordering can accidentally seed expensive exceptional geometry first.

The independent archived-cone audit supplies a useful measured connection:
protected root 14 (`111100000011100`, R<=11, no A cap) reaches protected roots
23 and 24, whose owners are the first two entries above. Root 15
(`111010100100101`, R<=2, A<=12) reaches 14, 23, and 24. The archived cones
contain 8,383 and 15,036 nodes respectively, overwhelmingly Route nodes.
This motivates a targeted diagnostic on those dependencies, but does not
authorize shrinking the frozen 183-query production scope or establish a bug.
Source: `TMP/root-blockers-gen6/receipt.json`, `.root_blockers.roots[].full`.

The local `census.py` only reads masks, the existing route-class assignment,
and saved byte counts. It uses bit-subset tests and counters, never symbolic
algebra or a replacement graph canonicalizer. `structural-census.json` contains
the complete four-loop census and five-loop intersection/pinch summaries.
The original class and matrix authority remains the existing RustRed/Symbolica
routing census and native route verification.

## Concrete input candidates

| Candidate | Mechanism | Changed input | Regeneration |
| --- | --- | --- | --- |
| Shared interface first | Seed the common parent interface selectively | Move the original BMW query block 33..35 before rows 0..32 | None |
| Dependency-ready cost | Offer strict descendants before parents; favor cheap shared ready owners | Permute complete owner blocks in rows 0..41 | None |
| Critical interface first | Start the measured dominant native work earlier | Move only banana's original block 6..8 to the start; keep all other blocks stable | None |
| Pinch-interface sector order | Rank BMW-equivalent prism pinch slots and the other parent's unique slot early in physical sector comparison | Reindex all family coordinates and inverse-map final coordinate ties | All 16 selected payloads in the reindexed four-loop family |

All three query-only candidates retain all 58 original query objects without any
field change, preserve each owner's relative row order, and leave the 16 broad
R12 anchor rows 42..57 in their exact original suffix. This avoids repeating
the previous helpers-first intervention, which moved those broad anchors
ahead of smaller anchors and changed initial representative selection.

The dependency-ready candidate constructs an edge from an owner to every
class in its strict labelled downset. Among owners whose descendant classes
have already been emitted, it minimizes `saved_bytes/(upstream_classes+1)`,
with exact rational comparison and mask tie-break. Saved bytes are only a
cost proxy. Its owner order is:

```text
0111100000 0111110000 0111010101 0111100100
0111111000 0111100110 1111110100 1111111000
0111100001 0111110010 0111111100 0111111001
1111111100 0111111110 1111111110 0111111111
```

The sector experiment chooses `sigma(new_slot)=old_slot`:

```text
sigma       = [0,3,4,9,1,2,5,6,7,8]
inverse     = [0,4,5,1,2,6,7,8,9,3]
CLI tie permutation = 0,4,5,1,2,6,7,8,9,3
old prism 1022 -> 1110111111, nonpositive index 3
old K3,3   511 -> 0111111111, nonpositive index 0
```

The priority is externally derived from the pinch groups, not topology names
inside the engine. An earlier slot changes relative sector lexicographic
priority when two sectors of equal line count differ there. It does **not**
mean the walker necessarily executes that pinch first.

`pinch-interface-sector-order/family.toml` preserves the family name and loop
momentum order but permutes the denominator rows and target powers.
`queries.json` permutes owner masks and lower/upper arrays, preserving query
IDs as physical query identities and every scalar bound. Inverse remapping
recovers all 58 original rows exactly. `routes.proposal.json` bijectively
remaps all 508 source/owner masks and retains the existing two loop-momentum
witness matrices per route verbatim. Those are 4x4 loop-basis matrices:
conjugating them with a ten-coordinate permutation would be incorrect.
Native `symmetry::verify` and `integral_transport::compile` reconstruct and
verify the corresponding denominator map in the new coordinates.

The proposal intentionally omits a usable family fingerprint and owner payload
list. Regenerate the two roots with the same sparse/depth-2/finite-search
policy and the inverse tie permutation, select the same physical 16 owner
supports by their remapped masks in the new checkpoint manifests, and obtain
new byte counts/hashes/fingerprint natively. Old native ordinals are invalid.
Never mix old-family and reindexed-family payloads or reuse old checkpoints.
The existing pinned `regenerate_owners.py` asserts old digests and ordinals;
it must not be invoked unchanged for this experiment.

This is a genuine sector-priority intervention, but not a perfectly isolated
one: symbolic-index/coefficient variable registration and sector enumeration
also change with family coordinates. Inverse tie priority restores physical
integral-coordinate tie comparisons, not those additional representation
effects. Label results accordingly.

## API and cross-owner descent audit

`IntegralOrder::with_permutation` in `solver/index.rs` explicitly changes only
the final denominator/numerator coordinate ties. Support count, original-slot
numeric sector lexicographic comparison, cuts, total degree and numerator
degree are decided first. Thus merely passing `--permutation 0,9,...` cannot
be advertised as choosing which support to pinch. `family-candidates` and
`family-close` already expose this tie control; changed rules need generation
and certification, not a rebuilt CLI. Coefficient variable order is a separate
Rust-side representation option, not a substitute sector control.

Reordering selection owners or route records does not choose another route:
the admitted maps are keyed by source support, duplicates are rejected, and
each selected library here has one owner per equivalence class. Changing owner
choice requires an alternate valid payload/map, not array shuffling. Query
array order can change initial scheduling and which valid covering record
becomes available first; it changes no physics or route identity.

Different tie orders **can** coexist across owners of the same common family.
This follows from the transition checks, not just loader permissiveness:

1. The exact evaluator requires each rule child to strictly descend under
   that owner's persisted order (`candidate_reduction/evaluator.rs`). The
   symbolic domain visitor separately calls `prove_wide_descent_with_limits`
   with that owner's order (`owners/domains/applied/engine.rs`).
2. Equal-support rule children remain Apply nodes of the same owner. A child
   entering Route must be a strict subsupport and lose active-line count.
3. Transport admission/worker checks preserve source-to-owner active-root
   cardinality. A full-root image goes Route->Apply; only a strict smaller
   image reenters Route. Literal owners cannot be redirected.

Hence support count, Route/Apply phase, and the applicable owner's local
well-founded order provide a piecewise descent argument. A same-cardinality
Apply->Apply change of owner is not allowed. This allows a cheaper one-owner
tie/pivot pilot in the original family if desired, keeping the other 15
payloads, but it is a different experiment from sector reindexing. Loader
compatibility still requires the common family, rank/finite-case and source
policy; a flattened single-order artifact has stricter ordering requirements.

An independent `sector_permutation` engine option would require consistent
changes to solver comparison, persisted `OrderingPolicy`/stable encoding,
exact descent/certification, bundle/checkpoint binding, and CLI/Rust/Python
interfaces. Do not make that broad change merely to run this bounded study.

## Measured-cost qualification and bounded BMW pivot preparation

The actual saved native records of the original matched four-loop controls
give the following cost census. This includes Ready G2 anchor inspections and
Epoch G2 residual inspections, whose record-kind strings differ.

| Original input | Native inspections | Summed native-record seconds | Banana Apply inspections / seconds | Banana fraction |
| --- | ---: | ---: | ---: | ---: |
| Ready r2 | 22,601 | 15.4236 | 20 / 10.2529 | 66.48% |
| Epoch rolling FIFO r2 (P2) | 31,846 | 16.4605 | 24 / 9.5576 | 58.06% |

These overlapping per-inspection wall seconds are neither process CPU seconds
nor cold reinspection timings. Route contributes 1.3756 seconds over 22,332
Ready inspections and 2.3822 seconds over 31,558 Epoch inspections. The banana
owner `0111100001` remains the dominant native cost despite its tiny record
count. Exact file hashes and per-owner totals are in `owner-costs.json`, read
from the saved `cold-control-{ready,rolling-fifo}-r2/four-all/checkpoint`
record files under `TMP/codex-rolling-measurements.eBFGjR/runs/`.

The bytes-based dependency order moves banana from block 4 to block 9,
original rows 6..8 to new rows 18..20, while keeping its R12 anchor at row 45.
The shared-BMW seed moves it only to block 5, rows 9..11. Neither is an early
banana test. The added `critical-interface-first` candidate moves precisely
rows 6..8 to the start; its SHA-256 is
`5efe336b141d921c903ce4316c88c705023c87e032084027fcb6043bfec4b7af`.
Its mechanism is to overlap dominant native work sooner. The opposing effect
is an oldest-prefix publication stall; unchanged or worse native+cold time
despite earlier overlap falsifies the proposed benefit. No further permutation
portfolio is implied by this measured-cost correction.

For a cheaper pivot diagnostic, root selected the nontrivial shared BMW
interface, not the degenerate four-line maximum-fan-in owner. Preparation files
`bmw-pivot-commands.json` and `bmw-pivot-pipeline.sh` define a same-source natural
baseline and exactly two pinch-class tie priorities:

| Arm | `--permutation` | Mechanistic contrast |
| --- | --- | --- |
| Natural baseline | `0,1,2,3,4,5,6,7,8,9` | Same-root, same-binary regenerated control |
| Cheap pinch class first | `1,2,7,8,3,4,5,6,0,9` | First active tie group pinches to `0111111100` |
| Costly pinch class first | `3,4,5,6,1,2,7,8,0,9` | First active tie group pinches to `0111111001` |

The two groups are directly derived from BMW's existing routed single-pinches;
natural order is retained within each group and inactive coordinates remain
`[0,9]`. The target-class saved byte counts are 104,601 and 1,226,920. Their
actual Epoch Apply costs also differ, 0.0831 versus 0.2074 seconds over two
records each (Ready 0.0733 versus 0.2091). This supports a cost contrast but
does not assert tie priority controls the first actual pinch, or predict a
speedup; pivots can instead change rational coefficient size and exceptional
guards. BMW itself accounts for only two Apply inspections and 0.1105 native
seconds in P2, so direct BMW evaluator savings alone cannot close the total
performance gap. Any useful gain must affect downstream geometry or sharing.

All three commands use the unchanged original family, `--nonpositive-indices
0,9`, sparse backend, depth 2, finite-case search, explicit identical finite
and case budgets, no rank restriction, and W16. The public CLI generates the
BMW root **downset**, not only one literal sector. Only its BMW single-sector
checkpoint shard may replace the selected BMW payload afterward; the other
15 selected payloads, all 508 routes, all 58 query rows/roles and their order
stay fixed. Locate that shard by exact mask in the generated manifest, not the
old ordinal 326. A same-source natural baseline is essential because the old
BMW shard came from the larger K3,3 generation and is not that control.

The command arrays retain a clearly invalid binary placeholder until the
final freeze is supplied. They use both existing locks, CPUs 32..47, a
1,740-second whole-arm soft deadline including admission (at most 1,800 with
the kill grace), a 1,500-second guarded pipeline
deadline, and stage limits of 600 seconds generation, 600 certification, and
120 cold artifact inspection. Timeout is a censored result, not permission
to increase a budget. No commands have run. The existing guard requires at
least 250 GiB available at start and stops below 150 GiB host headroom.

Before any stitched performance test, require source certification and cold
artifact load to succeed. Compare the `campaign inspect` master-key sets in
the same physical coordinates, both the whole BMW-downset set and the subset
whose positive support is exactly BMW. Keep terminal policy and finite-search
budgets unchanged. Report added/removed terminal keys and counts explicitly;
a faster variant with an enlarged or otherwise changed admitted terminal set
is not an equivalent-task speedup without a separate equivalence argument.
After stitching, native route verification and full all-source/all-root cold
closure remain mandatory. No old owner/checkpoint is overwritten.

## Fair smallest gate

After the current build, matched four-arm gate, and two cold mutation controls
have drained, run one fresh full four-loop Ready/Epoch pair for each query-only
candidate, using the same frozen optimized binary, W16, resource envelope,
rule payloads, all 58 queries, and independent all-source cold verification.
Compare to the contemporaneous original-input Ready/Epoch controls. A candidate
must not be favored merely because one engine benefits; report native+cold
wall time, native inspections, Route/Apply counts, emitted work, peak memory,
and censored/incomplete outcomes. Repeat only a promising candidate in reversed
arm order before interpreting a small difference.

The reindex arm needs bounded root generation first, exact source certificate
checks, all route verification, and then the same complete 58-query pair.
Compare physical coverage after inverse remapping, not byte identity. Keep
finite-case policy and terminal admission policy fixed, and explicitly report
the inverse-mapped terminal key set difference and count: terminal inflation
must not be hidden as a speedup. Report rule/coefficient bytes, exceptional
branches or uncovered geometry, and generation/certification cost separately
from walk savings. The prior natural/reverse single-owner trial had fewer
rules but much larger coefficients and was slower, so rule count alone is
not an optimization metric.

No full five-loop regeneration or factorial order search is proposed. The
four-loop result decides whether an input heuristic deserves a bounded
five-loop diagnostic; the frozen 67-owner/183-query scope remains unchanged.
