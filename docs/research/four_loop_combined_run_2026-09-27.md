# One combined four-loop run over all four-loop owners (2026-09-27, revised)

Status (revised in c4l session 2, 2026-09-27 ~20-22 UTC; gate text and counts corrected in the fix round of
2026-09-28, see the last section): the inputs are built, validated and
now reproducible from committed files. `four-all` drains **deterministically under Ordered**
(30,159 natives, records strictly identical at W6, W24 and W96). Under Ready it is
**schedule-sensitive**: 7 of 13 runs drained at W96 (95 % interval 0.25-0.81), against 17 of 18
at W24 and 5 of 5 at W6. The one W24 non-drain ran on socket 1. Every non-drain entered the same
self-sustained flood one rank above the anchors. The session-1 statements "drains robustly" and "orders in
which each owner's first row is a full orthant drained in all 13 runs" are **withdrawn**.
`four-all` is an **additional** four-loop control. It does not replace the C-4L gate (the
per-family FG/BMW/H/X controls). Its Ready results are reported as statistics over repeats,
never as a binary drain gate.
Evidence labels: **[M]** measured, **[E]** estimate or interpretation. No family-closure,
termination or minimal-master claim is made (`family_closure_claim` is false everywhere).

Inputs, commands, receipts, digests, the full results table and how to use `four-all` next to
the C-4L gate are in
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
| `four-all` (physics + rank-12 anchors) | 30,159 at every width, strictly identical records | 5/5 drained, 24.5-26.1 k | 17/18 drained, 21.9-24.4 k (7/8 on socket 1; 1 rank-13 flood) | 7 of 13 drained (21.7-24.1 k); the others flood at rank 13 |
| `four-all-r14anchors` | W24: 38,173 | - | 2/2, 29.0-29.4 k | 2/3 drained; rank-15 flood otherwise |
| `four-all-r13anchors` | W24: 33,750 | - | 1/1, 26.6 k | 9/11 drained (25.5-31.7 k); the other two flood at rank 14 = anchor + 1 |
| `four-all-h993r14` | W24: flood (730,570 natives at 900 s) | - | 0/1 (flood moved to two other owners) | - |
| `four-all-p5` (five-loop generation policy) | W6, W24 and W96: 31,717, strictly identical records | - | 2/2, 22.9-23.3 k | 2/3 (22.2-25.3 k); rank-13 flood otherwise |
| `four-all-a19` (box-first envelope) | W6 and W24: identical flood | 0/1 | 2/3 | 1/1 |
| `four-all-physics` | - | - | 0/1 (1 h) | - |
| `four-all-r6anchors` | - | - | 3/3, 12.5-13.4 k | not run |

Drained runs have 0 frontiers and pass the audit. The closure verifier
(`walk-verify-closure --require-closure`, full re-inspection, oracle binary 89558210) passes on
Ordered W24 and W96 and on three Ready W96 runs. In all of them, 32/32 initial records are
independently verified, 0 successors are uncovered and there are 0 containment disagreements
(`TMP/c4l-s2/verify/`). With the current oracle binary 46d4dd28, which reports the gate fields, the
Ordered W24 reference passes the gate with 32/32 roots independently verified
(`TMP/c4l-s2/fix/verify-46d4dd28-ordered-w24_four-all.json`; fix round). In drained runs, 98-99 % of natives are Route inspections, and the
non-entry banana owner takes 71-86 % of native seconds over about 20 natives. W96 rows ran on
socket 1 with 27-88 % foreign load, so their timings are void.

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
- Session B2 (8 more `four-all`, 8 rank-13 and 3 `four-all-p5` Ready W96 runs, 60 k early stop,
  `TMP/c4l-s2/post_b2.json`): all 7 non-drains show the same signature at the stop. There are
  1,500-13,755 Apply natives of `0111110010` at anchor rank + 1, against about 30 in a drained run,
  in the same two flavours:
  - point-like: 13.3-13.8 k on `0111110010` alone;
  - wide boxes: 1.5-1.7 k on it plus 2.8-3.0 k on `0111111001`.

  At the stop the flood does not yet dominate native seconds: `0111110010` takes 18-57 % of them in the
  B2 non-drains, and 9.5 % in session C's one W24 non-drain (rep8: 12,457 rank-13 Apply natives on
  `0111110010`, 12,459 over all ranks).
- The same owner floods in every other non-drained run: `four-all-a19` Ready W6 and Ordered
  W6 (rank 13), `four-all-r14anchors` Ready W96 (rank 15), and `four-all-physics` Ready W24
  (rank 5 = helper rank + 2, with `0111111001`). The fix round broke down 15 more drained runs of
  sessions B2 and C, for 19 drained breakdowns in all (`TMP/c4l-s2/fix/bd-*.json`). In 18 of them
  `0111110010` has exactly 30 Apply natives at anchor rank + 1, with lower corners `A_lo` <= 5. The
  exception has 188 (`A_lo` up to 16): the `-r13anchors` run B2 rep11, which reached rank 16. `0111111001` has 0 in all 19.

Why the rank-12 anchors do not stop it [M for the facts, E for the causal chain]:
- An owner orthant at rank r contains only rank <= r. Its own inspection emits rank r+1 and r+2
  descendants. The maximum scheduled finite rank is anchor rank + 2 for anchors 12, 13, 14 and 16
  in 55 of the 56 drained runs with such anchors. With the three drained `four-all-a19` runs, whose
  orthants are also rank 12, it is 58 of 59. The exception is one rank-13 Ready W96 run (B2 rep11),
  which reached rank 16.
- Those descendants are admitted as ordinary domains, and a later one aliases only if an earlier
  admitted box contains it. Which wide rank-13 boxes are admitted first depends on completion
  order. In the drained W96 run, the first wide box of `0111110010` left the line-2 dots
  unbounded. In rep2, the first wide boxes bounded line-2 dots at 1 or 2, which left
  (dots1 >= 1, dots2 >= 2) uncovered, and the point-like chain started there.
- Ready publishes in completion order, which differs from run to run. Ordered is deterministic:
  `four-all` takes the draining branch at every width, while `four-all-a19` takes the flooding
  branch at every width tried (Ordered W6 and W24: the first 600,000 committed records are
  identical, ignoring `seconds`).
- Session 1's partial-initial-overlap explanation (the old "engine note") is not needed: rep2
  had 0 partial initial inspections. Partial overlaps may add to the race; they are not required
  for it.

## Input-level fixes tested [M]

- **Anchors at rank 13, 14 or 16.** The uncovered layer only moves up. With rank-14 anchors the
  drained runs show the rank-12 picture shifted by two. Ready W96 still fragments: 2 of 3 runs
  drained, and the third was stopped at 900 s with 1,996,696 natives, 690,152 of them rank-15
  Apply natives of `0111110010`. Ordered W24 work rises from 30,159 to 38,173 natives.
  Rank-13 anchors (session B2): 9 of 11 Ready W96 runs drained, and the other two flood at rank
  14 = anchor + 1 on `0111110010`. That is not distinguishable from `four-all` (7/13, Fisher exact
  p = 0.21).
- **A rank-14 helper for `0111110010` alone.** The flood moves. The run fragments already at
  Ready W24 (1.38 M natives in 900 s), now on `0111100100` and `0111100000` at rank 14. Their
  input is the helper's rank-14 subsector descendants, which arrive through routes.
- [E] No finite anchor rank closes the gap, and rank-free helpers left frontiers
  (`four_loop_helper_bounds_2026-09-25.md`). Reliable draining under Ready is an engine property
  (which boxes are admitted, in what order), not something inputs can fix.

## Adversarial audit of the lane (session 2) [M unless stated]

- **Digests.** All 53 SHA-256 lines that README section 6 had at the time matched the files. After the fix
  round the block has 133 lines, all verified at commit time (see the last section). The 16 owner payloads
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
  (+5.2 %), Ready W24 22,855 and 23,250, max rank 14, 0 frontiers; `walk-verify-closure
  --require-closure` with full re-inspection PASS on the Ordered run (0 uncovered of 2,231,270
  successors). So the mismatch is small for this control. At Ready W96 (session B2) it drains
  in 2 of 3 runs (22,152 and 25,292 natives), and the third floods at rank 13 on `0111110010`, as
  with the Vakint-policy owners.
- **Helper shapes.** The five-loop plan-v3 has 67 helpers, 54 of them with a positive-power
  bound, and **no** rank anchors above the planner helper ranks (2-14). `four-all` adds rank-12
  anchors that the campaign does not have, and its helpers carry no positive-power bound. The
  campaign-shaped variant is therefore `four-all-physics`, which floods here on the same owner.
- **Corrections to earlier summaries.** "Drains robustly" and "all 13 runs" are refuted (6 of 13
  Ready W96 runs of `four-all` did not drain; the helper-first order does not protect). The
  tail-sample "fed by Route records" is reversed (see above). The four-all-physics hot-owner
  shares 74.0 % / 22.3 % are confirmed (74.05 % / 22.29 %). The four-all-a19 Ready W96 run had
  32 partial initial inspections (session 1 quoted 31-74 across runs).

## Consequences

1. **How to use `four-all`, in addition to the C-4L gate** (README section 5, revised in the fix round).
   - C-4L itself is unchanged: the per-family FG/BMW/H/X controls (the zero-Route arm).
   - **C-4L-comb-O** (deterministic schedules).
     - For record-keeping changes: `four-all` Ordered at W6 and W24, and at W96 on socket 1 under
       `TMP/locks/socket1.lock` with the recorder. Pass requires drained, audit PASS, 0 frontiers,
       strict record identity with the reference (30,159 natives, 65,444 records), and the
       verify-closure gate PASS (`verdict == PASS`, `roots_independently_verified == roots_total`,
       full re-inspection, current oracle binary). The reference passes that gate with 46d4dd28
       (32/32).
     - For changes that alter records on purpose (the v3 epoch engine, work-volume levers): the
       engine's deterministic schedule, i.e. v3 `Lockstep` with canonical resolution, or legacy Ordered.
       Pass requires drained, audit PASS, 0 frontiers, the same verify-closure gate, byte-identical
       records across W6, W24 and W96, and a **one-sided** upper guard of natives <= 33,175
       (1.1 x 30,159), or the lever's own pre-registered expectation. Natives and scheduled domains
       are reported against the Ordered 30,159 and against the legacy Ready distribution, 21,730-26,065
       (29 drained runs).
     - The earlier two-sided ±10 % band is withdrawn. All 29 drained legacy Ready runs lie below its
       lower edge, and G2' (scheduled domains 0.79-0.80x) would fail it by design.
   - **C-4L-comb-R** (Ready or v3 Rolling). This is a pre-registered one-sided Fisher exact test of the
     drain fraction, candidate against the frozen legacy 4a17f9c7 Ready, at alpha = 0.05 per width, W in
     {6, 24, 96}.
     - n >= 13 runs per arm and width (20 recommended at W96), interleaved in one session on one CPU
       set, each capped at 15 min or stopped at 60,000 natives.
     - The queue file is the pre-registration, and its digest is logged.
     - Promotion is blocked by:
       - a candidate frontier, violation or unknown non-drain signature;
       - a regression (p < 0.05 at any width);
       - a failed verify-closure gate on a drained candidate run.

       Mismatched n or CPU sets void the comparison. Two gate-PASS verify-closure reports per width
       are required.
     - At n = 13 against a legacy 7/13, a regression is declared for <= 2/13 and an improvement for
       >= 12/13. The superseded n >= 5 could not fail at W96 (0/5 against 7/13: one-sided p = 0.054).
     - The historical legacy table is context only: width and host are confounded in it.
     - `tools/comb_r_test.py` implements the rule.
   - **Known legacy non-drain signature, quantitative.** max(N993, N1009) >= 1,000, where N993 and N1009
     are the Apply natives of `0111110010` and `0111111001` at anchor rank + 1 at the stop. Other
     anchor + 1 owners are allowed. Measured: drained runs 30 (one run 188) and 0; 60 k-stopped
     non-drains 2,813-13,755 on the larger of the two.
   - Every run records the recorder: foreign busy CPUs, schedstat run delay, user instructions,
     IPC, instructions per native, and (from the fix round, committed `tools/run_c4l.py`) user and
     system seconds, faults, context switches and the effective user clock.
   - Keep `four-all-physics` as the small, campaign-shaped stress case. A v3 engine that drains
     it within the 1-hour rule would be direct evidence on the five-loop bottleneck.
2. **Engine requirement for v3** [E]. The flood is a coverage race on descendants just above the
   anchor rank. A canonical admission order makes the outcome reproducible. It does not
   guarantee drainage (`four-all-a19` Ordered floods). Two things need an explicit gate on this
   control: drainage of the anchor rank + 1 layer, and a detector for chains that sit outside
   every admitted box. Early recognition [M]: the ten non-drained runs that ran past 60 s had
   64-261 k natives at 60 s, and no drained run of any variant exceeded 38,173 natives. This supports
   the 60 k early stop. At 20 s the ranges overlap (non-drains 29-101 k; a drained Ordered r14 run had
   37.5 k), so no earlier rule is stated. The session-2 statement "41-58 k at 20 s and 181-479 k at
   120 s" fitted only three W96 runs and is withdrawn.
3. **Five-loop reading.** The five-loop hot owner `011101110111000` (connected, V4min 3,
   helper rank 3) resembles the four-loop flood owner (connected, V4min 2, helper rank 3) [E].
   Raising helper ranks is not a cure at four loops; it only moves the layer. So the input lever
   "raise low helper ranks" from session 1 is weakened. It is not refuted at five loops.
   Cross-check on the v2 campaign [M]: `tools/records_breakdown.py` over the five committed-records
   sidecars of the gen-7 clone (45,889,639 records, 27.47 M natives; copies in
   `TMP/c4l-s2/v2-gen7-records/`, outputs `bd-v2-gen{3..7}.json`, 65 s) gives the following.
   - Across all owners, only **33.4 %** of Apply natives lie above their owner's helper rank.
   - For the hot owner (helper rank 3, A <= 13, 63.6 % of native seconds) the figure is
     **40.7 %**.
   - The rest lie at or below the helper rank, where 54 of the 67 plan-v3 helpers carry an A
     bound. [E] These are mostly escapes in A, not in rank; this was not checked domain by
     domain.
   [E] So the four-loop mechanism (the layer above the anchor rank, which no box covers) explains
   at most a third to two fifths of the five-loop Apply work. `four-all`, whose helpers are
   unbounded in A, does not exercise the A-escape part. That part is exercised only by a variant
   with A-bounded helpers, which has not been built.

## Open issues

- The Ready W96 drain fraction of `four-all` is 7/13 (95 % Clopper-Pearson 0.25-0.81), against
  17/18 at W24 (Fisher exact p = 0.012). Unreliable Ready drainage is measured at W24 (1 of 18, on
  socket 1) and W96 (6 of 13). At W6, 5/5 drained, and all W6 runs were on socket 0.
  - The cause (width or host) is not resolved. Every W96 run was on socket 1: session 1 (1/2, load
    not recorded), session A (2/3, 27-45 % foreign load) and session B2 (4/8, 57-79 busy CPUs).
  - W24 on socket 1 (session C) gave 7/8, with one rank-13 flood. That differs neither from W24
    elsewhere (10/10, p = 0.44) nor from W96 (p = 0.17).
  - Separating width from host needs larger n on socket 1 at W24, or W96 on a quiet socket. The
    interleaved comb-R design removes the confound from candidate-versus-legacy comparisons only.
- The 10x slowdown of the drained session-C rep5 (92 s traversal, 1 % foreign load, at least 60 s of
  non-user on-CPU time) is unexplained. The extended recorder can attribute a recurrence.
- `four-all-r6anchors` was not run at W96. (`four-all-p5` Ordered is width-invariant at W6/W24/W96; fix round.)
- The physics helpers carry no positive-power bound. The five-loop campaign bounds A for 54 of
  its 67 helpers. The A-bounded variant has not been built.
- Owner decision pending: D-memo item D-C4L (README section 5, end). It covers the scope substitution
  (physics class plus anchors instead of the A <= 19 envelope) and the Vakint against five-loop
  generation policy (`four-all-p5`).
- The master plan and v3 design must name C-4L-comb-O and C-4L-comb-R next to C-4L. The fix round
  prepared that amendment on this branch as a separate commit, for the integrator.
- Not attempted: numerical sign conventions (`q^2-1` here against the thesis's Euclidean
  `q^2+1`), and exhaustive uniqueness of the common basis (a scratch search plus a structural
  argument, TMP/c4l).

## Fix round (2026-09-28): verifier findings and what changed [M unless stated]

An independent verifier (`TMP/progress/verify-c4l.md`) raised ten problems. All ten were checked
against the files and found real. Their dispositions:
1. **C-4L-comb-O band (major).** All 29 drained legacy `four-all` Ready runs (21,730-26,065 natives)
   lie outside ±10 % of 30,159, and G2' would fail the band by design. The two-sided band is withdrawn.
   The replacement requires drained, audit PASS, 0 frontiers, the verify-closure gate PASS
   (`roots_independently_verified == roots_total`) and width invariance under Lockstep, canonical, or
   Ordered. It adds a one-sided guard of natives <= 33,175 or a pre-registered per-lever expectation.
   Natives are reported against both 30,159 and the Ready distribution. The Ordered reference was
   re-verified with the current oracle binary 46d4dd28: PASS, 32/32 roots independently verified, 177 s,
   `assert_oracle_pass.py` GATE-PASS.
2. **C-4L-comb-R decision rule (major).** The rule is now pre-registered: n >= 13 per arm and width,
   interleaved with legacy 4a17f9c7 Ready runs in one session on one CPU set, and a one-sided Fisher test
   at alpha 0.05 per width. It states BLOCK, VOID and INCOMPLETE outcomes and maps Rolling to comb-R and
   Lockstep to comb-O. `tools/comb_r_test.py` implements it; `--self-test` reproduces every p-value
   quoted, and a smoke run over the session-A/B2 runs exercised the audit, signature and verify paths.
   The historical table is kept as context only (host-confounded).
3. **Session attribution (minor).** W96 was 1/2 in session 1 (load not recorded), 2/3 in session A
   (27-45 %) and 4/8 in session B2. "No load effect seen" is replaced by "not detectable at this n;
   session 1 unrecorded".
4. **Counts (minor).** README section 6: 95/95 digests verified before the fix; the block now also
   carries the `four-all/` inputs, the committed tools and the fix-round evidence (all lines verified at
   commit time). Max rank = anchor + 2: 55/56 drained runs (58/59 with `-a19`). Session C rep8: 12,457
   rank-13 Apply natives (12,459 over all ranks) and 9.5 % of native seconds, outside the B2 18-57 %.
5. **Pinning (minor).** The committed tools are `tools/run_c4l.py` (self-contained; argv rewriting
   tested identical to the shared runner), `tools/session.py`, `tools/ready_stats.py`,
   `tools/comb_r_test.py` and `tools/command-four-all.json`. The README now documents regenerating the
   Ordered reference (record identity, not file digest). The shared runner's comments were corrected; the
   pinned version is saved as `TMP/fable51-controls/run_control.8dace33c.py`.
6. **Over-general claims (minor).** Unreliable Ready drainage is now stated for W24 and W96 only (W6 5/5,
   socket 0 only). The 20-s and 120-s "recognisable early" ranges are withdrawn: at 20 s, non-drains had
   29-101 k natives and a drained run 37.5 k. The 60 s / 60 k justification stands (64-261 k natives
   at 60 s, drained maximum 38,173).
7. **Signature threshold (minor).** Known iff max(N993, N1009) >= 1,000 at anchor + 1; other anchor + 1
   owners are allowed. The baseline was confirmed on 15 more drained breakdowns (19 in all): 30 in 18,
   188 in one, and N1009 = 0 in all.
8. **Slow run (minor).** Session C rep5 (92 s against 7.9-10.8 s; at least 60 s of non-user on-CPU
   time) is documented and flagged by `ready_stats.py`. The recorder now records user and system seconds,
   faults, context switches and the effective user clock.
9. **Owner decision (minor).** D-memo item D-C4L (README section 5, end) puts the scope substitution
   and the Vakint against p5 policy to the owner. New measurement: `four-all-p5` Ordered W6 = 31,717
   natives, and W96 (session F, 23 s of socket-1 hold) = 31,717 natives. Records are strictly identical to
   Ordered W24 (68,483 records) at both widths.
10. **Governing documents (minor).** A separate commit on this branch adds C-4L-comb-O and C-4L-comb-R
    to the master plan's control table and to the engine-affecting gates (1.1, 1.2, 2.1, 4.4, §6 check 4,
    M-drain), next to C-4L. It also adds the schedule-to-control mapping to the v3 design (§3.4, S6).
    "When socket 1 is free" is replaced by "on socket 1 under socket1.lock with the recorder" (socket 1
    stays shared, HANDOFF §0.1 item 11).
