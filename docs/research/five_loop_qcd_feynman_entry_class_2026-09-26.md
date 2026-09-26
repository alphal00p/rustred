# Five-loop QCD Feynman-gauge entry class: planned roots for the 67 saved owners (2026-09-26)

Status: Package A of `FABLE_5_1_five_loop_vacuum_plan.md` (planner, checker,
match summary, tests, inputs). Everything below is either **measured** (a run
directory and file digests are cited) or **derived** (a formula applied to
measured classification data). Nothing here is a coverage, termination or
closure claim: native admission remains the coverage authority, the walk
still retains every descendant of every root (descendants are never clipped),
and `family_closure_claim` stays `false` in every artifact. No ETA is given.

## 1. Physics class (user decision, plan section 2)

Notation: A = total positive (denominator) power, R = total numerator rank,
D = A - R, t = active line count of an owner, V4 = number of quartic vertices of
a realization, L = 5, K = 0 gauge-parameter powers (Feynman gauge). The
dimension of a five-loop vacuum integral is 20 - 2A + 2R, so log-divergent
coefficients have D = 10 and the dimension-2 gluon self-energy term has D = 9.

- Gauge: Feynman; all Z's via gluon/quark/ghost self-energies and the
  ghost-gluon vertex with one external momentum nullified, in the
  auxiliary-mass (all lines massive) tadpole scheme.
- Connected entry owners: one root at exact D = 10 with A <= 16 - V4min,
  R <= 6 - V4min, and one at exact D = 9 with A <= 14 - V4min, R <= 5 - V4min
  (generic form: R_max = D - L + 1 - V4min + K, A_max = R_max + D).
- Factorized entry owners (matroid direct sums): one nested full-jet root
  A <= 24 - V4min, R <= 15 - V4min, D >= 9 (5L - 1 - V4min + K, 3L - V4min + K,
  2L - 1).
- Non-entry owners: two convenience roots with the widest connected boxes
  (V4 = 0): D = 10: A <= 16, R <= 6; D = 9: A <= 14, R <= 5.
- Per-coordinate uppers in local coordinates (active x = n - 1, inactive
  y = -n): A_max - t on active axes, R_max on inactive axes.
- Helpers: one full orthant per owner (lower 0, upper null, no D bound) at
  max_numerator_rank = the owner's largest root R_max and unbounded positive
  power; a finite max_positive_power is added only for owners whose
  matching-only diagnostic flags unresolved helper pieces
  (`summarize_owner_domain_match.py` -> `--helper-positive-power-owners-from`).
  Helpers precede their owner's roots in query order.

## 2. Classification method (generic in L and the vertex-degree set)

`examples/python/plan_renormalization_entry_queries.py` realizes each owner
by contracting the missing slots of a validated parent-vertex witness
(`examples/input/tide_five_loop_parent_vertices.json`, four parents
transcribed from `tide_five_loop_census.md`; every vertex sums to zero against
the momenta of `examples/input/tide_five_loop_manifest.json`, each slot occurs
twice with opposite signs, E - V + 1 = 5, connected, momentum rank 5), closes
its 2-isomorphism class (Whitney twists, cleavings, identifications) and
records the connected members as canonical unlabelled multigraphs. Connected
bridgeless five-loop vacuum skeletons with vertex degrees in {3, 4} are
enumerated by backtracking over symmetric multiplicity matrices, deduplicated
by canonical form, and reduced by merging every series class to one edge; a
skeleton whose reduced form lies in an owner's class certifies the owner as
entry-capable at that skeleton's V4. V4min is the minimum over matching
skeletons. The factorized flag is the census's (manifest
`representatives[].factorized` joined through `selection.json
owners[].representative`).

Measured enumeration (run `TMP/qcd-feynman-d9d10-input.dcgP73/plan-v1`,
`timing.json`: classification 85.9 s, whole planner command 1m27.6s on one
core of CPUs 200-207): labelled leaves 138 / 1,004 / 9,236 / 100,525 /
1,256,395 and distinct skeletons 10 / 38 / 64 / 38 / 16 for V4 = 4, 3, 2, 1, 0;
101 owner canonical forms; every reduced skeleton matched some owner. These
equal the audit's `skeleton_result2.json` for all 67 owners field by field
(t, entry_capable, V4min, skeleton_counts_by_V4), and the committed fixture
`examples/python/fixtures/tide_five_loop_skeleton_classification.json`
(sha256 `6f5a6c5bb8b0ceee0bff412dd45130d07baa002f39c6a1edb83d761ed2bfb70f`)
is that document; the production run recomputed it and required equality.

Result: 67 owners = 41 connected entry + 18 factorized entry + 8 non-entry
(`000011001001011`, `001000100101111`, `010111011000001`, `011001110110100`,
`011011111101001`, `101010000110001`, `110010101101011`, `111000100011101`).
Connected V4min histogram {0: 16, 1: 5, 2: 13, 3: 4, 4: 3}; factorized
{1: 7, 2: 4, 3: 4, 4: 3}. Queries: 183 = 67 helpers + 41 x 2 + 8 x 2 + 18 x 1.

## 3. Classification table (measured class/V4min; derived bounds)

Roots list A_max / R_max and the difference band; the helper rank is the
owner's largest root R_max; every helper has unbounded positive power in v1.

| # | Owner mask | t | Class | V4min | Roots (kind: A_max/R_max/D) | Helper rank |
|---:|---|---:|---|---:|---|---:|
| 1 | `000011001001011` | 6 | non-entry | - | conv: A<=16, R<=6, D=10; conv: A<=14, R<=5, D=9 | 6 |
| 2 | `011011000111111` | 10 | connected | 2 | phys: A<=14, R<=4, D=10; phys: A<=12, R<=3, D=9 | 4 |
| 3 | `101010000110001` | 6 | non-entry | - | conv: A<=16, R<=6, D=10; conv: A<=14, R<=5, D=9 | 6 |
| 4 | `010111011000001` | 7 | non-entry | - | conv: A<=16, R<=6, D=10; conv: A<=14, R<=5, D=9 | 6 |
| 5 | `001000100101111` | 7 | non-entry | - | conv: A<=16, R<=6, D=10; conv: A<=14, R<=5, D=9 | 6 |
| 6 | `011001110110100` | 8 | non-entry | - | conv: A<=16, R<=6, D=10; conv: A<=14, R<=5, D=9 | 6 |
| 7 | `010011111101011` | 10 | connected | 2 | phys: A<=14, R<=4, D=10; phys: A<=12, R<=3, D=9 | 4 |
| 8 | `111001100111111` | 11 | connected | 1 | phys: A<=15, R<=5, D=10; phys: A<=13, R<=4, D=9 | 5 |
| 9 | `111011100111111` | 12 | connected | 0 | phys: A<=16, R<=6, D=10; phys: A<=14, R<=5, D=9 | 6 |
| 10 | `111011101100011` | 10 | factorized | 1 | nested: A<=23, R<=14, D>=9 | 14 |
| 11 | `111001111101011` | 11 | connected | 0 | phys: A<=16, R<=6, D=10; phys: A<=14, R<=5, D=9 | 6 |
| 12 | `111011111101011` | 12 | connected | 0 | phys: A<=16, R<=6, D=10; phys: A<=14, R<=5, D=9 | 6 |
| 13 | `111000010101001` | 7 | connected | 4 | phys: A<=12, R<=2, D=10; phys: A<=10, R<=1, D=9 | 2 |
| 14 | `111000100111001` | 8 | connected | 4 | phys: A<=12, R<=2, D=10; phys: A<=10, R<=1, D=9 | 2 |
| 15 | `111100000011100` | 7 | factorized | 4 | nested: A<=20, R<=11, D>=9 | 11 |
| 16 | `111010100100101` | 8 | connected | 4 | phys: A<=12, R<=2, D=10; phys: A<=10, R<=1, D=9 | 2 |
| 17 | `111000100011101` | 8 | non-entry | - | conv: A<=16, R<=6, D=10; conv: A<=14, R<=5, D=9 | 6 |
| 18 | `011001000101111` | 8 | connected | 3 | phys: A<=13, R<=3, D=10; phys: A<=11, R<=2, D=9 | 3 |
| 19 | `111101010011100` | 9 | connected | 3 | phys: A<=13, R<=3, D=10; phys: A<=11, R<=2, D=9 | 3 |
| 20 | `011101111101100` | 10 | connected | 2 | phys: A<=14, R<=4, D=10; phys: A<=12, R<=3, D=9 | 4 |
| 21 | `111001100111001` | 9 | connected | 3 | phys: A<=13, R<=3, D=10; phys: A<=11, R<=2, D=9 | 3 |
| 22 | `011101111111100` | 11 | connected | 1 | phys: A<=15, R<=5, D=10; phys: A<=13, R<=4, D=9 | 5 |
| 23 | `111101111111100` | 12 | connected | 0 | phys: A<=16, R<=6, D=10; phys: A<=14, R<=5, D=9 | 6 |
| 24 | `000010011001001` | 5 | factorized | 4 | nested: A<=20, R<=11, D>=9 | 11 |
| 25 | `001100101110000` | 6 | factorized | 4 | nested: A<=20, R<=11, D>=9 | 11 |
| 26 | `001001000101111` | 7 | factorized | 3 | nested: A<=21, R<=12, D>=9 | 12 |
| 27 | `010100101110000` | 6 | factorized | 3 | nested: A<=21, R<=12, D>=9 | 12 |
| 28 | `011001000001111` | 7 | factorized | 3 | nested: A<=21, R<=12, D>=9 | 12 |
| 29 | `110001101101001` | 8 | factorized | 3 | nested: A<=21, R<=12, D>=9 | 12 |
| 30 | `001001100101111` | 8 | connected | 2 | phys: A<=14, R<=4, D=10; phys: A<=12, R<=3, D=9 | 4 |
| 31 | `011110111001001` | 9 | connected | 2 | phys: A<=14, R<=4, D=10; phys: A<=12, R<=3, D=9 | 4 |
| 32 | `110010101101011` | 9 | non-entry | - | conv: A<=16, R<=6, D=10; conv: A<=14, R<=5, D=9 | 6 |
| 33 | `111001000001111` | 8 | connected | 2 | phys: A<=14, R<=4, D=10; phys: A<=12, R<=3, D=9 | 4 |
| 34 | `110001011101011` | 9 | connected | 2 | phys: A<=14, R<=4, D=10; phys: A<=12, R<=3, D=9 | 4 |
| 35 | `110110111101001` | 10 | connected | 2 | phys: A<=14, R<=4, D=10; phys: A<=12, R<=3, D=9 | 4 |
| 36 | `001001011101001` | 7 | factorized | 2 | nested: A<=22, R<=13, D>=9 | 13 |
| 37 | `111111001001001` | 9 | connected | 0 | phys: A<=16, R<=6, D=10; phys: A<=14, R<=5, D=9 | 6 |
| 38 | `111011000101001` | 8 | connected | 2 | phys: A<=14, R<=4, D=10; phys: A<=12, R<=3, D=9 | 4 |
| 39 | `111001100101101` | 9 | connected | 2 | phys: A<=14, R<=4, D=10; phys: A<=12, R<=3, D=9 | 4 |
| 40 | `111100011111100` | 10 | connected | 2 | phys: A<=14, R<=4, D=10; phys: A<=12, R<=3, D=9 | 4 |
| 41 | `101011111001010` | 9 | connected | 2 | phys: A<=14, R<=4, D=10; phys: A<=12, R<=3, D=9 | 4 |
| 42 | `111100111101001` | 10 | connected | 2 | phys: A<=14, R<=4, D=10; phys: A<=12, R<=3, D=9 | 4 |
| 43 | `101101100101000` | 7 | factorized | 2 | nested: A<=22, R<=13, D>=9 | 13 |
| 44 | `011100111100001` | 8 | factorized | 2 | nested: A<=22, R<=13, D>=9 | 13 |
| 45 | `111001100110110` | 9 | connected | 0 | phys: A<=16, R<=6, D=10; phys: A<=14, R<=5, D=9 | 6 |
| 46 | `011101011001001` | 8 | factorized | 1 | nested: A<=23, R<=14, D>=9 | 14 |
| 47 | `101011011101001` | 9 | factorized | 1 | nested: A<=23, R<=14, D>=9 | 14 |
| 48 | `111010111100011` | 10 | connected | 1 | phys: A<=15, R<=5, D=10; phys: A<=13, R<=4, D=9 | 5 |
| 49 | `111101110101001` | 10 | connected | 0 | phys: A<=16, R<=6, D=10; phys: A<=14, R<=5, D=9 | 6 |
| 50 | `011000101101011` | 8 | factorized | 1 | nested: A<=23, R<=14, D>=9 | 14 |
| 51 | `011011101101001` | 9 | connected | 0 | phys: A<=16, R<=6, D=10; phys: A<=14, R<=5, D=9 | 6 |
| 52 | `011011111101001` | 10 | non-entry | - | conv: A<=16, R<=6, D=10; conv: A<=14, R<=5, D=9 | 6 |
| 53 | `110101101101001` | 9 | factorized | 2 | nested: A<=22, R<=13, D>=9 | 13 |
| 54 | `111100100101001` | 8 | factorized | 1 | nested: A<=23, R<=14, D>=9 | 14 |
| 55 | `011111001101001` | 9 | factorized | 1 | nested: A<=23, R<=14, D>=9 | 14 |
| 56 | `111101101101001` | 10 | factorized | 1 | nested: A<=23, R<=14, D>=9 | 14 |
| 57 | `111111001101001` | 10 | connected | 0 | phys: A<=16, R<=6, D=10; phys: A<=14, R<=5, D=9 | 6 |
| 58 | `111111011101001` | 11 | connected | 0 | phys: A<=16, R<=6, D=10; phys: A<=14, R<=5, D=9 | 6 |
| 59 | `111011001101001` | 9 | connected | 0 | phys: A<=16, R<=6, D=10; phys: A<=14, R<=5, D=9 | 6 |
| 60 | `111011011101001` | 10 | connected | 0 | phys: A<=16, R<=6, D=10; phys: A<=14, R<=5, D=9 | 6 |
| 61 | `101111101101001` | 10 | connected | 0 | phys: A<=16, R<=6, D=10; phys: A<=14, R<=5, D=9 | 6 |
| 62 | `111111101101001` | 11 | connected | 0 | phys: A<=16, R<=6, D=10; phys: A<=14, R<=5, D=9 | 6 |
| 63 | `111110111001001` | 10 | connected | 1 | phys: A<=15, R<=5, D=10; phys: A<=13, R<=4, D=9 | 5 |
| 64 | `111101111101001` | 11 | connected | 0 | phys: A<=16, R<=6, D=10; phys: A<=14, R<=5, D=9 | 6 |
| 65 | `111011111101010` | 11 | connected | 1 | phys: A<=15, R<=5, D=10; phys: A<=13, R<=4, D=9 | 5 |
| 66 | `111111111101001` | 12 | connected | 0 | phys: A<=16, R<=6, D=10; phys: A<=14, R<=5, D=9 | 6 |
| 67 | `011101110111000` | 9 | connected | 3 | phys: A<=13, R<=3, D=10; phys: A<=11, R<=2, D=9 | 3 |

## 4. Receipts (measured)

Evidence directory `TMP/qcd-feynman-d9d10-input.dcgP73/` (commands, timings and
all digests in its `RESULTS.md`; produced by the planner with
`--classification-fixture` and `--executable target/release/rustred`, binary
sha256 `32fdec098a57dd0c51aef71c01c260fb5cf7d0954b0992db08bb0f955aed358a`,
receipt git head `01f8db5dcb20465ff557ae5cf5aec272259f120c`):

- `plan-v1/queries.json` (121,640 bytes) sha256
  `74bd5301e9d0dcd02d66a921762d80b25141e4736b473ce122551c07aba0ca0c`
- `plan-v1/entry-plan-receipt.json` sha256
  `898ac05ef8beac741404dba104fc270bd9bf6648961d95adaaa5cf77902a4bda`
- `plan-v1/skeleton-classification.json` sha256
  `6f5a6c5bb8b0ceee0bff412dd45130d07baa002f39c6a1edb83d761ed2bfb70f`
- `plan-v1/entry-plans/*.json`: 14 budget groups counted by
  `rustred entry-domain-plan` (exact); 271,990,954,170 finite starting targets
  in total, of which the four nested factorized groups hold 271,883,043,264
  (A <= 23/R <= 14 alone 169,246,051,632) and the ten exact-D groups
  107,910,906. Starting-target counts are input sizes, not walk work.
- Checker `check_renormalization_entry_queries.py`: status pass in 1.7 s;
  every bound re-derived from the receipt, exact v2 row shape, helpers first,
  each root contained in its helper (`Domain::contains` semantics), closed-form
  counts equal to the Rust counts for all 116 roots (closed form validated on
  the 1,324-tuple control `011101110111000`, A <= 11, R <= 2, D >= 9), 23,200
  membership probes with zero box/physical disagreements.
- Determinism: a first scratch run and the production run produced
  byte-identical `queries.json` and `skeleton-classification.json`; the receipt
  differs only in the recorded argv/executable path (tested at L = 2 with a
  fake executable: all outputs except `timing.json` byte-identical).

Inputs: `examples/input/tide_five_loop_parent_vertices.json` sha256
`bc0737dd6b5520cb869502fc68421ffbde3a9842100fec2113952956f594ff70`;
`examples/input/tide_five_loop_manifest.json` sha256
`1b7d5bc9516621ef49110a3b81e43513ffca0f4fd18c80e11d47abc2c9a1dd81`;
`campaigns/five-loop-dependency-closure/inputs/selection.json` (read-only)
sha256 `d2667dc9c761d1dcd171408452eea030c0b1eae23421e55407745d046deec3f0`.

## 5. What is not established

- The matching-only diagnostic (validation ladder step (b)) has not been run
  on these queries; helpers may still need a finite A after it.
- Whether the symbolic walk terminates on this class is unknown; the
  single-owner pilot is the first evidence and is outside this note.
- The walk retains every escaping Apply and Route obligation; nothing in the
  planner or checker bounds descendants, and `family_closure_claim` is false.
