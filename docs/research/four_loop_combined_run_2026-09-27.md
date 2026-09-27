# One combined four-loop run over all four-loop owners (2026-09-27, revised)

Status (revised in c4l session 2, 2026-09-27 ~20-21 UTC): the inputs are built, validated and
now reproducible from committed files. `four-all` drains **deterministically under Ordered**
(30,159 natives, records strictly identical at W6, W24 and W96). Under Ready it is
**schedule-sensitive at W96**: 3 of 5 runs drained; the others entered a self-sustained
flood one rank above the anchors. The session-1 statements "drains robustly" and "orders in
which each owner's first row is a full orthant drained in all 13 runs" are **withdrawn**.
Evidence labels: **[M]** measured, **[E]** estimate or interpretation. No family-closure,
termination or minimal-master claim is made (`family_closure_claim` is false everywhere).

Inputs, commands, receipts, digests, the full results table and the recommended gate are in
[`examples/input/four_loop_combined/README.md`](../../examples/input/four_loop_combined/README.md).
Raw evidence: `TMP/c4l-build.wBU9zC/` (session 1), `TMP/c4l-s2/` (session 2) and
`TMP/fable51-controls/c4l-4a17f9c7-*`.

## Decision: common basis, not a multi-family harness

The owner asked for one combined run that resembles the five-loop campaign as closely as
possible. Two paths were investigated.

- **Common basis (chosen).** Luthe's A4 basis contains both cubic four-loop roots: the prism
  (1022, the Vakint H parent) and K3,3 (511, the X parent). The FG and BMW parents are
  contractions of these roots.
  - The engine needs no change. The owner loader already takes owners from several roots, as in
    the five-loop run.
  - Every labelled sector is routed to one class owner, so the walk exercises Route, integral
    transport and multi-root loading. These dominate five loops (Route is 75-78 % of five-loop
    natives). The per-family Vakint controls had zero Route records.
- **Multi-family harness (rejected for now).** A family index in the owner key would cost 5-7
  agent-days on the legacy engine, for code that is frozen and then deleted. It still would not
  exercise Route. If a future need arises, it belongs in the v3 epoch engine at W2.0/S2
  (+1-2 agent-days) [E].

## What was built [M]

- **Owner programs.** Regenerated in A4 with the release binary 4a17f9c7 through the path that
  produced the five-loop owner shards (`family-candidates --checkpoint-dir`), with the Vakint
  package policy (sparse, numerical depth 2, finite-case search, no rank scope).
  Root 1022: 314 sectors, 22,715 rules, 386 residuals. Root 511: 328 sectors, 22,228 rules,
  445 residuals. 281 zero sectors, as in thesis Table 9.1.
  - **Reproducible byte-for-byte** (session 2): two regenerations reproduce all 642 sector
    programs, both manifests and both candidate bundles, hence the 16 owner payloads, in about
    1.4 min (16 cores). `regenerate_owners.py` automates it and checks the pins; the payloads
    stay uncommitted.
- **Selection.** 16 class owners (Table B.1) and 508 labelled routes, 492 transported (five
  loops: 67 owners, 8,246 routes, 8,179 transported).
- **Routing validation.** Census reproduced byte-for-byte; an independent Symbolica replay,
  generic in L, passes with 28 corruption controls rejected; RustRed verifies all 508 routes at
  load and refuses 8 corrupted selections.
- **Queries.** The planner physics class at `--loops 4 --gauge feynman --difference-set 7,8`
  (9 connected, 6 factorized, 1 non-entry owner; 42 queries; 61,683,424 starting tuples; checker
  pass) and the historical A <= 19 / R <= 12 / D >= 7 envelope (362,692,824 tuples). Matching
  diagnostics find 0 unresolved, 0 gap and 0 invalid pieces for every variant.

## Results [M]

Full table in the README (section 5). Summary:

| Family | Ordered (W6 / W24 / W96) | Ready W6 | Ready W24 | Ready W96 |
|---|---|---|---|---|
| `four-all` (physics + rank-12 anchors) | 30,159 at every width, strictly identical records | 3/3 drained, 24.5-26.1 k | 10/10 drained, 22.3-24.4 k | 3 of 5 drained (21.7-23.8 k); the others flood at rank 13 |
| `four-all-r14anchors` | W24: 38,173 | - | 2/2, 29.0-29.4 k | 2/3 drained; rank-15 flood otherwise |
| `four-all-r13anchors` | W24: 33,750 | - | 1/1, 26.6 k | 3/3 drained (25.5-25.9 k); max rank 15, so not a fix by construction; small sample |
| `four-all-h993r14` | W24: flood (730,570 natives at 900 s) | - | 0/1 (flood moved to two other owners) | - |
| `four-all-p5` (five-loop generation policy) | W24: 31,717 | - | 2/2, 22.9-23.3 k | not run |
| `four-all-a19` (box-first envelope) | W6: flood (1 run) | 0/1 | 2/3 | 1/1 |
| `four-all-physics` | - | - | 0/1 (1 h) | - |
| `four-all-r6anchors` | - | - | 3/3, 12.5-13.4 k | not run |

Drained runs have 0 frontiers and pass the audit; 98-99 % of their natives are Route
inspections, and the non-entry banana owner takes 71-86 % of native seconds over about 20
natives. W96 rows ran on socket 1 with 27-78 % foreign load, so their timings are void.

## The fragmenting mode

The full per-owner breakdown of the non-drained Ready W96 run (9.12 M committed records,
`TMP/c4l-s2/bd-ready-w96-rep2-four-all.json`) [M]:
- 5.19 M natives but only 687.6 native-seconds in 3,605 s of wall: coordinator-bound.
- Owner `0111110010` (class 993, connected, V4min 2): 1,712,757 Apply natives, 75.3 % of native
  seconds, all but two at **rank 13**, one above its rank-12 anchor. Secondary rank-13 sweeps on
  `0111100100` (26,742) and `0111100000` (2,222).
- The flood feeds itself. Only 1,013 Route natives route into `0111110010`; the 3.44 M Route
  natives are its subsector descendants routed to other owners (the handoff's tail-sample
  "fed by" wording had the direction reversed).
- Its domains are point-like in the dots of lines 1 and 2 and unbounded in the dots of line 3.
  The front advances steadily (largest line-2 dots 245 → 1,072 over the ten id-deciles, natives
  per decile 226 k → 150 k). [E] It fills a two-dimensional lattice region with no visible
  boundary; nothing suggests it would drain.
- The other non-drained Ready W96 run (rep5, 1.58 M natives at its 900 s cap) floods the same
  layer with wide boxes of growing bounds instead of points: 58,977 rank-13 natives on
  `0111110010` take 73.3 % of native seconds, with rank-13 sweeps on `0111111001`, `0111100100`
  and `0111100000`.
- The same owner floods in every other non-drained run: `four-all-a19` Ready W6 and Ordered
  W6 (rank 13), `four-all-r14anchors` Ready W96 (rank 15), and `four-all-physics` Ready W24
  (rank 5 = helper rank + 2, with `0111111001`). In the drained runs checked (Ready W96,
  Ordered W24, and the rank-14 variant at W24) it has about 30 Apply natives at anchor
  rank + 1, with small lower corners.

Why the rank-12 anchors do not stop it [M for the facts, E for the causal chain]:
- An owner orthant at rank r contains only rank <= r. Its own inspection emits rank r+1 and r+2
  descendants. The maximum scheduled finite rank is anchor rank + 2 for anchors 12, 13, 14 and 16.
- Those descendants are admitted as ordinary domains, and a later one aliases only if an earlier
  admitted box contains it. Which wide rank-13 boxes are admitted first depends on completion
  order. In the drained W96 run, the first wide box of `0111110010` left the line-2 dots
  unbounded. In rep2, the first wide boxes bounded line-2 dots at 1 or 2, which left
  (dots1 >= 1, dots2 >= 2) uncovered, and the point-like chain started there.
- Ready publishes in completion order, which differs from run to run. Ordered is deterministic:
  `four-all` takes the draining branch at every width, while `four-all-a19` took the flooding
  branch in its only Ordered run.
- Session 1's partial-initial-overlap explanation (the old "engine note") is not needed: rep2
  had 0 partial initial inspections. Partial overlaps may add to the race; they are not required
  for it.

## Input-level fixes tested [M]

- **Anchors at rank 13, 14 or 16.** The uncovered layer only moves up. With rank-14 anchors the
  drained runs show the rank-12 picture shifted by two. Ready W96 still fragments: 2 of 3 runs
  drained, and the third was stopped at 900 s with 1,996,696 natives, 690,152 of them rank-15
  Apply natives of `0111110010`. Ordered W24 work rises from 30,159 to 38,173 natives.
- **A rank-14 helper for `0111110010` alone.** The flood moves. The run fragments already at
  Ready W24 (1.38 M natives in 900 s), now on `0111100100` and `0111100000` at rank 14. Their
  input is the helper's rank-14 subsector descendants, which arrive through routes.
- [E] No finite anchor rank closes the gap, and rank-free helpers left frontiers
  (`four_loop_helper_bounds_2026-09-25.md`). Reliable draining under Ready is an engine property
  (which boxes are admitted, in what order), not something inputs can fix.

## Adversarial audit of the lane (session 2) [M unless stated]

- **Digests.** All 53 SHA-256 lines of README section 6 match the files; the 16 owner payloads
  match `selection.json` in `gen/`, `inputs/` and the staged folder (48/48); the committed
  `four_loop_common_basis.toml` equals the generation input; the four-all query document is
  reproduced from `physics/queries.json` + staging (`d1ac816e...`).
- **Owner rule.** Recomputed from `selection.json`: every owner is the rule's choice (a class
  member in the root-511 downset first, then the representative's own sector, then the largest
  mask), 12 owners from root 511 and 4 from root 1022, class sizes sum to 508, 492 transported
  routes, each owner lies inside its generating root. The 134 shared sectors have **0**
  byte-identical programs between the two roots (the claim holds). [E] Consequence: owners
  generated under 1022 and 511 are different but equally valid programs; the walk never mixes
  them for one mask because each class has exactly one owner.
- **Generation policy.** The five-loop owners were generated with `--max-numerator-rank 10
  --exact-backend sparse-factorized --numerical-depth 0` (argv in
  `TMP/tide-r10-four-parent-batch.yGOAw6/parent-*/command.txt`); the four-loop owners here with
  sparse, depth 2, search, no rank scope. Regenerating the 16 owners with the five-loop policy
  (`TMP/c4l-s2/gen-p5/`, 21 s + 20 s) gives 22,505 / 21,885 rules and 596 / 797 finite residuals
  (Vakint policy: 22,715 / 22,228 and 386 / 445), owner bytes 3.78 MB instead of 6.56 MB (banana
  1.43 MB instead of 3.32 MB). `four-all-p5` drains with audit PASS: Ordered W24 31,717 natives
  (+5.2 %), Ready W24 22,855 and 23,250, max rank 14, 0 frontiers. So the mismatch is small for
  this control; it was not run at W96.
- **Helper shapes.** The five-loop plan-v3 has 67 helpers, 54 of them with a positive-power
  bound, and **no** rank anchors above the planner helper ranks (2-14). `four-all` adds rank-12
  anchors that the campaign does not have, and its helpers carry no positive-power bound. The
  campaign-shaped variant is therefore `four-all-physics`, which floods here on the same owner.
- **Corrections to earlier summaries.** "Drains robustly" and "all 13 runs" are refuted (2 of 5
  Ready W96 runs of `four-all` did not drain; the helper-first order does not protect). The
  tail-sample "fed by Route records" is reversed (see above). The four-all-physics hot-owner
  shares 74.0 % / 22.3 % are confirmed (74.05 % / 22.29 %). The four-all-a19 Ready W96 run had
  32 partial initial inspections (session 1 quoted 31-74 across runs).

## Consequences

1. **Recommended C-4L gate** (README section 5).
   - C-4L-O, required: `four-all` Ordered at W6 and W24 (W96 when socket 1 is free), drained,
     audit PASS and strictly identical to the reference records (30,159 natives, 65,444
     records). For the v3 epoch engine, which changes the records on purpose, use instead:
     drained, audit and verify-closure PASS, identical across widths, natives within ±10 %.
   - C-4L-R: `four-all` Ready W96 x3, 15-minute cap each. On the legacy engine this arm is
     diagnostic: a non-drain whose native seconds are dominated by Apply natives at anchor
     rank + 1 on the V4min-2 connected owners (`0111110010`, `0111111001`) is known behaviour.
     Any other signature, a frontier or a violation fails. For v3 the arm is required: 3/3
     drained, identical.
   - Keep the per-family FG/BMW/H/X controls as the zero-Route arm.
   - Keep `four-all-physics` as the small, campaign-shaped stress case. A v3 engine that drains
     it within the 1-hour rule would be direct evidence on the five-loop bottleneck.
2. **Engine requirement for v3** [E]. The flood is a coverage race on descendants just above the
   anchor rank. A canonical admission order makes the outcome reproducible. It does not
   guarantee drainage (`four-all-a19` Ordered floods). Two things need an explicit gate on this
   control: drainage of the anchor rank + 1 layer, and a detector for chains that sit outside
   every admitted box. The fragmenting runs are recognisable early [M]: 41-58 k natives at 20 s
   and 181-479 k at 120 s, whereas drained runs finish with 22-31 k natives in 9-20 s of
   traversal.
3. **Five-loop reading** [E]. The five-loop hot owner `011101110111000` (connected, V4min 3,
   helper rank 3) matches the four-loop flood owner (connected, V4min 2, helper rank 3). Raising
   helper ranks is not a cure at four loops; it only moves the layer. So the input lever
   "raise low helper ranks" from session 1 is weakened. It is not refuted at five loops.

## Open issues

- The Ready W96 drain rate comes from 5 runs of `four-all` and 3 of the rank-14
  variant. It is a small sample, not a rate estimate.
- `four-all-p5` and `four-all-r6anchors` were not run at W96.
- The physics helpers carry no positive-power bound. The five-loop campaign bounds A for 54 of
  its 67 helpers.
- Not attempted: numerical sign conventions (`q^2-1` here against the thesis's Euclidean
  `q^2+1`), and exhaustive uniqueness of the common basis (a scratch search plus a structural
  argument, TMP/c4l).
