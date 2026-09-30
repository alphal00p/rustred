# Mechanistic ordering inputs, 2026-09-29

Status: four query-order pairs and two banana-pivot generations completed;
no decisive performance gain established. The later stitched natural pair
closes; mixed-line-first grows dramatically and is censored in both engines.
Exact terminal-key equality remains unestablished and is not assumed. A cheap
finite-prefix proxy also completes but is slower. Full coordinate reindexing
now closes the complete control with reduced work in two matched pairs;
Ready/Epoch means16.937s/16.729s include cold verification and have mixed
pairwise signs, so there is no robust Epoch speedup. No production
mutation, graph canonizer, or algebra implementation. The
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
and candidate-policy validation, not a rebuilt CLI. Coefficient variable order is a separate
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
0,9`, sparse backend, depth 2, finite-case search, identical default finite
policy and explicit case budgets, no rank restriction, and W16. The public CLI generates the
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
deadline, and a 600-second generation limit. The prepared pipeline stops after
generation; native candidate inspection and the stitched full-58 campaign are
separate granted steps. Timeout is a censored result, not permission
to increase a budget. No commands have run. The existing guard requires at
least 250 GiB available at start and stops below 150 GiB host headroom.

The sparse backend constructs exact candidates after modular discovery, but
the generation report explicitly says `prepare-solve-save; no source replay
or closure certification`. This is the same candidate-policy authority as
the packaged baseline, not independent replay of the original IBP sources.
`certify-candidates` without degree bounds attempts unrestricted whole-downset
closure as well as source checking; it is not a required gate for this scoped
exploratory study, and natural BMW has no established passing receipt for that
stronger task. The core source-replay-only API is not exposed by the current
CLI, and the separate `PrunedExactMaterializer` path is not this generator.

Before any stitched performance test, require native candidate loading and
inspection through existing APIs. `campaign inspect` accepts a certified
artifact, not the uncertified candidate produced here; do not fabricate that
pipeline. The existing public candidate loader exposes retained terminal keys;
use that authority for a same-coordinate key-set comparison, both the whole
BMW-downset set and the subset whose positive support is exactly BMW. Keep terminal policy and finite-search
budgets unchanged. Report added/removed terminal keys and counts explicitly;
a faster variant with an enlarged or otherwise changed admitted terminal set
is not an equivalent-task speedup without a separate equivalence argument.
After stitching, native route verification and full all-source/all-root cold
closure remain mandatory, but validate the walk relative to its supplied
candidate rules, not independent replay of their original IBP algebra. No old
owner/checkpoint is overwritten.

## Dominant banana: one mixed-momentum promotion

The measured banana owner `0111100001` uses active zero-based slots
`[1,2,3,4,9]`: `k2`, `k3`, `k4`, `k1-k4`, and `k1-k2-k3`, respectively.
Their literal loop-momentum incidence counts are `1,1,1,2,3`; slot 9 is the
unique densest active line in this input basis. Promote only that slot:
`--permutation 9,0,1,2,3,4,5,6,7,8`, compared with same-root regenerated natural
`0,1,2,3,4,5,6,7,8,9`. The relative order of every other active or inactive
coordinate is unchanged. This is representation-aware input design, not a
graph-invariant edge ranking or a new algebra implementation.

The admitted routing table provides a second concrete distinction. Deleting
any of the five active lines reaches owner `0111100000`; deleting slot 9 is
already that representative (no transport), whereas deleting 1, 2, 3 or 4
uses a verified transport. This does **not** imply the new tie order pinches
slot 9 first: the actual sector ordering remains fixed. The falsifiable
hypothesis is that promoting the mixed line changes same-degree pivot choices
and simplifies the dominant banana recurrence or its emitted geometry. Its
58–66% share of measured native time offers greater direct leverage than
BMW's <1%. Larger coefficients, unchanged/worse banana Apply cost or total
native+cold time refute a useful gain; terminal inflation cannot count as one.

`banana-pivot-commands.json` and `banana-pivot-pipeline.sh` prepare exactly these
two bounded arms, with `--nonpositive-indices 0,5,6,7,8`, the same sparse/depth2/
finite-search/case/bundle/checkpoint policy and W16/CPU/lock/deadline guards as
the BMW commands. Both generations have completed, as recorded below;
terminal-set comparison and stitched full-58 verification remain pending.
Generation covers the banana root downset;
only the exact-mask root shard may replace the selected banana payload. The
other 15 owners, all 508 routes and all 58 query rows, roles and order remain
fixed. Original owner ordinal 297 is not assumed for new checkpoints.

Native candidate inspection/loading, rule/coefficient/byte/time costs and
exact terminal-key/count comparison precede the full selected-owner 58-query
native/cold gate. The sparse candidates have baseline-equivalent generation
authority, not independent original-source replay. Unrestricted
`certify-candidates` is not a mandatory step. Public `candidate_bundle inspect`
is an existing example adapter for counts, not terminal-key enumeration; exact
keys are available from the existing public loaded reducer API. The separately
reviewed minimal `candidate_bundle terminals ARITY BUNDLE` example extension
uses that loader and emits sorted raw keys, count, family and ordering; it is
not a new decoder, source verifier or normalizer. Its diagnostic build is
separate from the frozen timed generation/walk binary. Build/native validation
and exact set comparison must finish before asserting terminal-set equality.

CLI integration audit corrected both preparation scripts: explicit
`--finite-max-visited-points` / `--finite-max-retained-terminals` options are
only accepted with `retain-rank-finite`, even when values equal defaults. They
are therefore omitted under the baseline `search` policy. Do not switch to
finite retention to make those flags parse. Depth2/search/unrestricted scope
canonically encodes the packaged `ordinary-source-port-default-v1` policy;
mixed saved roots and tie orders are accepted, but the common saved solver
policy and family fingerprint must match exactly.

The two authorized banana generations completed on frozen `89d90a3a`:
natural/slot9-first took 6.331/6.428 native-reported seconds (9.161/9.154
guarded wall seconds), with six solved sectors and 16 aggregate finite
residuals each. Aggregate rule counts were 549/469, but aggregate coefficient
bytes increased from 2,604,135 to 2,965,222. These aggregate costs include the
whole generated downset, not only the replacement owner. The exact banana
mask is native ordinal 5 in both new manifests; its shard sizes are
3,322,117/2,695,401 bytes. Receipts and hashes are in
`banana-pivot-results.json` under the preparation artifact directory.
The separately bounded loader-only diagnostic reached its 120-second limit
without producing an executable; no retry or alternate decoder was used.
At this generation-only stage exact terminal-key comparison and the stitched
full-58 gate remained pending. These results alone establish neither equivalent
terminal inventories nor a walk-performance improvement. All three owned
process groups drained. The later stitched outcomes are recorded below.

## Fair smallest gate

A separately prepared smallest broad-anchor input pair keeps the original
array as its control and moves only original row 45 (banana R12) immediately
before original rows 6..8. Its file is
`banana-broad-anchor-first/queries.json`, SHA-256
`f0c9f41c703567e302f6893138a05987bf28d3ca7140c0331febc08123d7d35c`.
The complete 58-row multiset and explicit query roles are unchanged; all
other rows retain their relative order. Unlike the three earlier candidates,
this intentionally removes one row from the R12 suffix. The existing
`input-mapping.json` records the exact bijection and control/variant pair;
the frozen 28-arm execution matrix was not changed.

Containment is complete, not partial: row45 has the same owner, all local
lower bounds zero, all upper/A/D bounds absent, and rank12; rows6..8 have
rank5/5/4 and only additional restrictions. Ready `Queue::admit` and Epoch
`admit_initial_with` may map a later query to an already admitted containing
domain. In the original inputs, banana R5 already absorbs its two constrained
rows, so the expected immediate gain is **one** fewer initial root, not three.
Original saved initial banana records are R5/id3 and R12/id19; their native
times were Ready0.224/0.873s and Epoch0.241/0.894s. Most banana cost is later
work, so a decisive gain would require changed descendant reuse or geometry.

Initial domains themselves bypass G2 residual planning and initial-D-band
pruning. The plausible G2 benefit is instead a broader banana native becoming
published earlier for later eligible descendants. It can also stall an older
publication prefix or change ID/cut geometry adversely. Report admitted
initial inventory, initial/descendant banana Apply time, native inspections,
emitted events and full native+cold wall time; unchanged/worse work or time
falsifies the performance hypothesis. All 58 required query rows still need
exact cold verification; containment may change the initial-root count.
This pair completed after the original three query-order pairs, with no
claimed gain; the measured results are below.
An independent StageA review confirmed the exact row permutation, unchanged
full row objects and top-level metadata/roles, and the recorded SHA-256.

After the matched four-arm gate and two cold mutation controls drained, one
fresh full four-loop Ready/Epoch pair ran for each query-only candidate, using
the same frozen optimized binary, W16, resource envelope, rule payloads,
all 58 queries, and independent all-source cold verification.
Compare to the contemporaneous original-input Ready/Epoch controls. A candidate
must not be favored merely because one engine benefits; report native+cold
wall time, native inspections, Route/Apply counts, emitted work, peak memory,
and censored/incomplete outcomes. Repeat only a promising candidate in reversed
arm order before interpreting a small difference.

### Completed query-order measurements

The optimized `89d90a3a` executable has SHA256
`581dc252aaeffd647032a37c944df673aaa3c8e91120394d08ce0eb67679ae5c`.
All eight arms pass full cold-All verification with zero violations; none is
censored. These are one pair per candidate, not repeatable optimum claims.
Times charge native command and independent cold reinspection separately.

| Query order | Ready native / cold (s) | Epoch native / cold (s) | Epoch total difference |
| --- | ---: | ---: | ---: |
| Shared interface first | 9.786 / 15.170 | 11.808 / 13.156 | +0.03% (one noisy tie) |
| Dependency-ready cost | 8.998 / 13.173 | 11.180 / 13.157 | +9.77% |
| Critical interface first | 9.584 / 12.163 | 10.992 / 15.150 | +20.21% |
| Banana broad anchor first | 9.980 / 13.155 | 11.752 / 14.152 | +11.97% |

Broad-anchor admission reduces independently checked initial roots from32 to31
while retaining every one of the58 requested rows. Relative to original FIFO,
its saved domains fall51,139→49,038 (4.11%), but native inspections barely fall
31,846→31,667 (0.56%). Its25.904s total is not an improvement over original
FIFO's25.845s two-run mean. Other query-block permutations leave native work
essentially unchanged. No pair supports a decisive native speedup.

Full CPU, memory, work, contention and process-drain receipts are preserved in
`TMP/codex-verifier16-matrix.0xtHE0/QUERY_ORDER89_RESULTS.md` and
`query-order89-results.json`. The report's SHA256 is
`aff38f767d906e23f01a075ee8fd66fca6abc990c8a1e1729ea844e01a3ddcb7`;
the JSON's is`2dfd168684e0cded175fb15ae783c140db6b14cbc9d804cb2d38159f462000ea`.
Native-tree and cold-single-child RSS have different scopes and are not summed.
Sparse host-contention samples do not justify correcting these timings.

Source-boundary interpretation: both Ready and checkpointed Epoch finish
initial query admission before dispatching native work (`walking/mod.rs`
initial-admission loop; Epoch `restore/runtime/public.rs` admission stage and
`controller.rs`'s complete-admission precondition). Their full-containment
lookups can reuse an admitted live domain without requiring it to have been
inspected or recursively closed (`queue.rs::admit_with_lookup` and
`epoch/store.rs::lookup`). Thus all admitted initial geometry is available to
descendant full-containment lookup before any query-order scheduling advantage.
An unchanged inspection count across the first three block permutations is
consistent with limited additional full-cover sharing from solving an owner
earlier; it is not evidence that order can never matter. Initial maximal-cover
admission, representative/cut choices, later generated domains, partial G2
coverage and native-cost overlap can still change. This is a source-supported
interpretation, not a universal invariant. A pivot change is different: it can
change emitted rule/domain geometry, not merely the schedule of fixed inputs.

The reindex arm needs bounded root generation first, native candidate-policy
checks, all exact route verification, and then the same complete 58-query pair.
Compare physical coverage after inverse remapping, not byte identity. Keep
finite-case policy and terminal admission policy fixed, and explicitly report
the inverse-mapped terminal key set difference and count: terminal inflation
must not be hidden as a speedup. Report rule/coefficient bytes, exceptional
branches or uncovered geometry, and generation/validation cost separately
from walk savings. The prior natural/reverse single-owner trial had fewer
rules but much larger coefficients and was slower, so rule count alone is
not an optimization metric.

No full five-loop regeneration or factorial order search is proposed. The
four-loop result decides whether an input heuristic deserves a bounded
five-loop diagnostic; the frozen 67-owner/183-query scope remains unchanged.

## Final monitored build: stitched pivot experiment

Frozen native `3428b519` (SHA256
`321b02b166c61dae927a220b7b8007b4659fef009d2b5b003084830b0f43eca3`),
W16 on CPUs32–47, same58 required query objects and508 routes. Only the exact
banana-root payload is substituted; all15 other payloads remain byte-identical.
Both arms use the same depth2 finite-search generation policy. Exact selected
terminal-key equality has not been established and is not required to explore
this change: different finite terminal bases can be legitimate. Neither equal
aggregate counts nor cold traversal establish independent algebraic provenance.

| Input / scheduler | Result | Native seconds | Cold-All seconds | Domains | Native inspections |
| --- | --- | ---: | ---: | ---: | ---: |
| Fresh natural / Ready | Complete, cold-All PASS | 9.661 | 15.178 | 66,382 | 24,368 |
| Fresh natural / Epoch FIFO | Complete, cold-All PASS | 11.191 | 13.156 | 51,139 | 31,846 |
| Mixed line first / Epoch FIFO | Operator-stopped, censored | 129.177 | Not run | 1,445,109 | 1,191,008 |
| Mixed line first / Ready | Time-censored | 60.672 | Not run | 215,330 | 170,436 |

Natural total24.839 versus24.347 seconds is one near-parity pair, not a robust
speedup or permission to replace the original repeated performance gate.
Both complete all58 required queries/32 admitted roots; the cold loader reports
28 saved terminals across the16 owners, not banana-only terminals. Mixed arms
have no completed cold-loader terminal observation. The unfinished arms are not
proof of mathematical nonclosure and do not supply completion timings.

The generated six-sector bundle has fewer rules with the mixed tie order
(549→469), yet traversal grows drastically in both schedulers. Logged G2
records/residual plans reach458,008/456,673 in mixed Epoch and48,912/48,658 in
mixed Ready. This is evidence of much more fragmented/repeated work, not an
isolated causal proof of a G2 bug. It falsifies this particular ordering's
practical benefit and is not recommended for production. All owned jobs drained;
partial checkpoints and failed acceptance receipts are retained.

Evidence: `TMP/codex-banana-matrix.rSODf8/{RESULTS.md,results.json,counters.json}`;
input-only exact-mask stitcher in `TMP/codex-banana-stitch.4Lf4R3/`.

## Finite helper prefix: a limited rank-deepening proxy

The unchanged58 required rows receive16 additional finite auxiliary helpers
containing97 integer entry points, each contained in an original required row.
All original programs and routes remain unchanged. This tests cheap early narrow
work, **not** true staged checkpoint/rank deepening: initial admission still
completes before walking, and a later broad initial representative can absorb
a narrow helper.

The final Epoch run completes and cold-All verifies all74 input rows/48 admitted
roots. Native13.065s plus cold22.179s=35.244s, versus27.345s for the original
Epoch mean. Domains fall51,139→49,174 and inspections31,846→29,930, but total
time increases28.89%. The full pilot, including preparation/report/drain,
takes91.4s within its240s bound. This rejects only the cheap prefix approach;
it does not disprove true staged deepening. No production changes follow.

Evidence: `TMP/codex-final-monitor-matrix.LJLGfi/finite-prefix-falsifier/` and
`fourall-finite-prefix-depth0/queries.json`. More generic source/sector
discovery recipes are being implemented in an isolated worktree; the true
algebraic integral-order abstraction remains a separate design, not a delivered
runtime option in these measurements.

## Completed sector-priority coordinate reindex

The prepared pinch-interface coordinate permutation above was executed with
the same final3428 binary, without a Rust rebuild. Both parents were regenerated
using sparse exact/depth2/finite search; all16 selected payloads come from those
new native checkpoints. The508 routes were remapped and admitted natively:
492 transported witnesses were checked and16 routes are identities.
Inverse mapping recovers every original58 query object, including bounds and
roles, exactly. No old-family payload was mixed into the new fingerprint.

First-parent core generation took26.026s (guard27.169s), yielding314 sectors,
19,984 rules and386 aggregate finite residuals. Second-parent core total was
36.333s, yielding328 sectors,21,228 rules and445 aggregate finite residuals.
These are downset generation counts, not distinct selected-owner masters.
Generation costs are separate from the matched walking boundary below.

| Scheduler on the same reindexed library | Native seconds | Cold-All seconds | Sum | Domains | Native inspections |
| --- | ---: | ---: | ---: | ---: | ---: |
| Ready | 8.564 | 9.166 | 17.730 | 50,039 | 16,082 |
| Epoch FIFO | 9.665 | 7.174 | 16.839 | 36,013 | 19,568 |

Both complete all58 required queries through32 admitted roots and pass full
cold reinspection. The loader reports28 aggregate saved terminals for both,
with the same SearchFinite policy. This is not exact terminal-key identity
or an independent replay of the original IBP algebra. The whole pilot takes
438.03s within its900s inclusive limit; all eight owned process groups drain.

The reindexed input materially improves the observed end-to-end times versus
the original input, but **do not label old-input Ready versus new-input Epoch
as a scheduler speedup**. On identical new inputs the single Epoch pair is
about5% faster in native-plus-cold wall while native traversal itself is slower.
The coordinate change affects sector priority, representation and variable
registration together. This is a useful mechanistic direction, not an isolated
comparator proof or a demonstrated1.5x architecture gain. Evidence:
`TMP/codex-sector-reindex.D5Yawx/results.json`.

The separately authorized reverse-order repeat also completes, with exactly
the same saved library and no regeneration:

| Repeated scheduler (FIFO ran first) | Native seconds | Cold-All seconds | Sum |
| --- | ---: | ---: | ---: |
| Epoch FIFO |9.464 |7.155 |16.619 |
| Ready |7.984 |8.160 |16.144 |

Both again pass all58 required rows/32 admitted roots. Ready's additional
full event audit passes in4.150s, timed separately. The small scheduler
advantage changes sign: two-pair means are approximately16.729s Epoch versus
16.937s Ready, about1.23% apart, not a robust speed win. Native command wall
remains slower for Epoch in both pairs. The new input library improves the
observed whole native-plus-cold boundary versus the original library, but no
five-loop ordering, full-generation or1.5x architecture claim follows.
Evidence: `TMP/codex-sector-reindex.D5Yawx/reverse-replication/`.
