# One combined four-loop run over all four-loop owners (2026-09-27)

Status: inputs built and validated, and a combined walk drained with audit PASS
at W6 and W24. The W96 run on socket 1 is recorded in the input README. Evidence labels: **[M]**
measured, **[E]** estimate or interpretation. No family-closure, termination
or minimal-master claim is made.

Inputs, commands, receipts and digests are in
[`examples/input/four_loop_combined/README.md`](../../examples/input/four_loop_combined/README.md).
Raw evidence is in `TMP/c4l-build.wBU9zC/` and `TMP/fable51-controls/c4l-*`.

## Decision: common basis, not a multi-family harness

The owner asked for one combined run that resembles the five-loop campaign as
closely as possible. Two paths were investigated.

- **Common basis (chosen).** Luthe's A4 basis contains both cubic four-loop
  roots: the prism (1022, the Vakint H parent) and K3,3 (511, the X parent).
  The FG and BMW parents are contractions of these roots.
  - The engine needs no change. The owner loader already takes owners from
    several roots, as in the five-loop run.
  - Every labelled sector is routed to one class owner, so the walk exercises
    Route, integral transport and multi-root loading. These dominate five
    loops (Route is 75-78 % of five-loop natives). The per-family Vakint
    controls had zero Route records.
- **Multi-family harness (rejected for now).** A family index in the owner key
  would cost 5-7 agent-days on the legacy engine, for code that is frozen and
  then deleted. It still would not exercise Route. If a future need arises, it
  belongs in the v3 epoch engine at W2.0/S2 (+1-2 agent-days) [E].

## What was built [M]

- **Owner programs.** Owners were regenerated in A4 with the release binary
  4a17f9c7 through the path that produced the five-loop owner shards
  (`family-candidates --checkpoint-dir`), using the Vakint package policy.
  - Root 1022: 314 sectors, 22,715 rules.
  - Root 511: 328 sectors, 22,228 rules.
  - Wall time 58.6 s and 25.3 s, run concurrently on 16 cores each. The zero-sector count
    is 281, the same as thesis Table 9.1.
- **Selection.** 16 class owners (Table B.1) and 508 labelled routes, of which
  492 need numerator transport. The five-loop selection has 67 owners and
  8,246 routes, 8,179 of them transported.
- **Routing validation.**
  - The Symbolica census was reproduced byte-for-byte.
  - An independent Symbolica replay, generic in L, passes with 28 corruption
    controls rejected.
  - RustRed verifies all 508 routes natively at load and refuses 8 corrupted
    selections.
- **Queries.**
  - Physics class from the generic planner at `--loops 4 --gauge feynman
    --difference-set 7,8`, the L = 4 image of the five-loop D in {9,10} class:
    9 connected, 6 factorized and 1 non-entry owner; 42 queries;
    61,683,424 starting tuples. The checker passes.
  - The historical A <= 19 / R <= 12 / D >= 7 envelope, for comparability:
    362,692,824 starting tuples.
  - Matching-only diagnostics find 0 unresolved, 0 gap and 0 invalid pieces for
    every variant.

## Results [M]

`four-all` is the physics class plus 16 rank-12 owner orthants
(12 = 3L, the historical rank-only anchor).
- It drains robustly: 22-30 k natives, 7-17 s of traversal, peak RSS
  0.34 GB or less, at Ready W24 (5 runs), Ready W6 (3), Ordered W6 (2) and
  Ordered W24 (1).
- The audit passes in every run. The Ordered runs are record-identical under
  strict comparison across repeats and across W6 and W24, with 30,159 natives
  each, so the Ordered policy gives a width-invariant oracle.
- 98-99 % of natives are Route inspections. Apply inspections carry about
  92 % of native seconds.

`four-all-physics`, the planner rows alone, **does not drain within 1 h** at
Ready W24.
- At 3,600 s it had 2.37 M natives and 2.53 M discovered domains, with 11 of
  16 initial roots closed and zero frontiers. The run was coordinator-bound.
- 96.3 % of native seconds went to the two connected owners with V4min = 2 and
  helper rank 3. Nearly all of those natives were rank-5 descendants arriving
  above the helper rank.
- Appending rank-6 orthants makes it drain: 13 k natives, 4.4 s, 3 runs.

The historical envelope in box-first order is **schedule-sensitive**.
- Identical input bytes drain in 9 s in 2 of 3 Ready W24 runs.
- The other Ready W24 run exceeded 1.1 M natives, and both W6 runs exceeded
  2.7-3.2 M natives, with 20 of 32 initial records closed.

## Consequences

1. **The combined four-loop control for the next push is `four-all`**
   (`run_control.py --family four-all`). It is route-dominated, drains in
   seconds and passes audit, and it replaces the four C-4L family runs as a
   correctness oracle. Like the per-family runs, it is not a W48-W100 scaling
   test [E]; see the W96 row in the README.
2. **`four-all-physics` is the representative stress case** [E]. It has the
   planner's per-owner helper ranks, as in the five-loop campaign, and the
   same pathology: a low-helper-rank connected owner flooded by rank-escaping
   routed descendants. That makes it a small, cheap analogue of C-HOT for the
   v3 engine. A v3 engine that drains it within the 1 h rule would be direct
   evidence on the five-loop bottleneck.
3. **Input lever for five loops** [E]. At four loops, raising every owner
   orthant to the largest rank that its routed descendants carry removes the
   fragmentation: 24 k natives at rank 12, 13 k at rank 6.
   - The five-loop analogue raises low helper ranks on the connected owners
     with V4min >= 2.
   - It must be measured with a matching diagnostic and a P-FRESH pilot. It is
     not established here, and the cost of rank-escaping helpers at five loops
     is unknown.
4. **Engine note** [E]. When a full orthant overlaps an earlier non-orthant
   initial record, its coverage apparently depends on timing: it goes through a
   partial initial overlap inspection before descendants are admitted.
   - The v3 canonical mode should register all initial orthants before
     admitting any descendant.
   - Until then, query documents should keep each owner's full orthant first,
     which is the planner's helper-first order.

## Open issues

- The generation policy follows the Vakint packages: unrestricted rank,
  `--numerical-depth 2`, `search`. The five-loop owners were generated with
  `--max-numerator-rank 10`, `sparse-factorized` and depth 0. Rule shapes may
  differ.
- The physics helpers carry no positive-power bound, because no diagnostic
  asked for one. The five-loop campaign bounds A for owners with t >= 8.
- The owner rule prefers K3,3-root programs, so 12 of 16 owners come from root
  511. The 134 shared sectors have byte-different programs in the two roots.
- The owner payloads are in TMP and are not committed. They can be regenerated
  in about 1.5 min and are pinned by SHA-256 in `selection.json`.
- Not attempted: numerical sign conventions (`q^2-1` here against the thesis's
  Euclidean `q^2+1`), and exhaustive uniqueness of the common basis (a scratch
  search plus a structural argument, TMP/c4l).
