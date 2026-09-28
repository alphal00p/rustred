# W0.7 work-volume census: gate 0.7, G2' and union-cover bounds, cost laws, Q3 (2026-09-27)

Plan item W0.7 of `docs/research/fable51_next_push_master_plan_2026-09-27.md` (section 5, gate 0.7). It feeds
W0.11 and the D-session (decisions D2, D3, D4). Branch `fable_5_1-v3-census`. Labels: **[M]** measured (receipt file
and binary sha256 cited), **[E]** estimate or model output. Nothing here is an ETA or a closure claim
(`family_closure_claim` stays false). The potential check is observational only.

## 0. Verdicts

- **Gate 0.7: PASS [M].** Threshold [E]: at least 30% of Apply CPU must sit in domains whose residual, against
  natives merged before dispatch, is at most 10% of their points in at most 8 pieces. Measured on v2 generation 7
  with 6000 draws proportional to native seconds:
  - 70.1% of Apply CPU passes with today's D-band cuts (D-only);
  - 89.0% passes with one box-hull piece, which is also today's vocabulary;
  - 84.8% passes with (A, R) rectangles, which need C2;
  - 61.6% of Apply CPU sits in domains that the merged natives cover completely.

  The pass also holds at generations 6 and 3, with a 300 s dispatch margin, and with 10x the sampling precision
  (T1). W4.2 (G2' residual anchors) may proceed on this criterion.
- **Projected Apply cost of residual-only inspection [E]:** 0.100 of today's Apply seconds with D-only cuts, 0.057
  with the hull, 0.057 with A/R. The model applies the per-owner binned cost law to the residual pieces of the same
  domain set. It does not model the changed successor set. For comparison, the W0.9 oracle measured 0.328x
  inspections on the drained C-HOT closure for pure union-cover residuals.
- **C2 adds little on top of the hull [M/E].** The hull residual, expressible today, reaches a higher gate share than
  the C2 A/R cuts (89.0% vs 84.8%) at the same projected cost (0.057). Adding C2's lower bounds to the hull
  (`hull_c2`) gives 90.6% and 0.048. This is an input to D3.
- **The "hot-owner pilot" C-HOT is CPU-dominated by owner 000011001001011, not 011101110111000 [M]** (section 6).
- **Importing the hot-owner closure gains about nothing [M]** (section 7).
- **Escape shells do not dominate [M]** (section 9), so the conditional P-anchor lever has no trigger.
- **The cost-exponent gap is reconciled [M]** (section 10): 0.74-0.90 and 0.51-0.66 are two estimators, a bin-mean
  law and a per-native geometric law.
- **Q3: guards are almost entirely affine in the indices; coefficient denominators are not [M].** The duplicated
  denominator role was a construction artefact (section 12).

## 1. Inputs, binaries, method

Receipt: `/common/dev/rustred/TMP/w0/census/receipt-v4/` (`batch.log` exit 0 at 20:48 UTC, `q3-batch.log` exit 0).
It supersedes `TMP/w0/census/receipt/`, which was produced by binary census-v3 built from an unverified tree and
without the conservative accounting described below. Two Q3-only attempts stopped at the rustred inner-pool
preflight (exit 4) and are kept in `receipt-v4/q3-attempt{1,2}/`.

| item | value |
|---|---|
| census binary | `.claude/worktrees/fable51-compact/TMP/census-v4`, sha256 `876974d9bbe5669d85969c2b0e52286eb8175d4af91161a540b9e18e88acd07d`, built from `7a36c606` (`receipt-v4/census-build.txt`) |
| Q3 binary | `.claude/worktrees/fable51-compact/TMP/rustred-q3-9d7c8865`, sha256 `9d7c8865ea298ebb722cce216254a1777247709d14e581d50618b50d2335bcd3`, built from `7a36c606`; the only uncommitted tracked change is the required `vendor/symbolica` heap-pow patch on 953e26e2 (`receipt-v4/q3/rustred-build.txt`) |
| v2 checkpoints | block clones of CP5 generations 3, 6 and 7 of `campaigns/five-loop-qcd-feynman-d9d10-v2` (`.claude/worktrees/fable51-compact/TMP/gen{3,6,7}`; each `latest.json` is byte-identical to `TMP/v2-checkpoint-copy-gen{3,6,7}`), plus the heartbeat series of run `20260926T151353.794886Z` |
| pilot (C-HOT) | `TMP/qcd-feynman-d9d10-pilot-hot-owner/matrix-32fdec/hot-owner-physics-ordered/run/result.json`: binary 32fdec, W6, Ordered, 3 queries in owner 011101110111000 (helper A <= 13, R <= 3; physics D = 10 and D = 9), drained, 7,767,543 records |
| controls | four-loop FG/BMW/H/X: `TMP/fable51-controls/census-4l-4a17f9c7/<f>/checkpoint`. C-5F: `TMP/fable51-controls/wave2-profile/w50-new-ready/five-finite/checkpoint` |
| Q3 input | `campaigns/five-loop-qcd-feynman-d9d10-v2/inputs/selection.json`, read-only; sha256 d2667dc9..., identical to the interim and original campaigns' manifests, so owner masks mean the same coordinates in the pilot and in v2 |
| CPUs | 16-31,272-287. Census at nice 19, builds at nice 5. Wall times are listed for provenance only and are not measurements |

**Method** (`tools/research/census/README.md`). A query Apply domain Q is tested point by point against anchor
domains of the same (phase, owner): every lattice point is tested up to `--cap` points, and above that `--samples`
uniform points are tested.

Anchor sets:
- `natives_before_dispatch`: Apply natives committed before Q's reconstructed dispatch time (commit time minus
  native seconds minus a wait margin, read on the heartbeat series). This is the G2' anchor rule.
- `natives_before_commit`: Apply natives committed before Q's own record.
- `all_earlier_ids`: every Apply domain with a smaller ID. This is the general union-cover bound (D2).
- For pending queries: `all_natives` (G2' at resume) and `all_other_domains` (D2 at resume).

Residual vocabularies:
- `d_only`: maximal D bands that hold uncovered points. This is the `initial_overlap` mechanism.
- `hull`: Q intersected with the box hull of the uncovered points, plus A and R caps and a D band. One piece, in
  today's vocabulary (per-coordinate lower and upper bounds, `max_positive_power`, rank, D range).
- `ar_c2`: (A, R) rectangles. Needs the C2 lower bounds.
- `hull_c2`: the hull plus C2 lower bounds.
- `exact_pointwise`: the uncovered points themselves. This is a lower bound, not a vocabulary.

Samples:
- `hist_pps_seconds`: committed Apply natives drawn with probability proportional to native seconds, so a share of
  draws estimates a share of Apply CPU.
- `hist_uniform`: committed Apply natives drawn per inspection.
- `pending_pps_predicted`: finite pending Apply domains drawn proportional to a predicted cost [E] from the binned
  cost law.
- `pending_uniform`: finite pending Apply domains drawn uniformly.

**Changes since the first receipt** (commits on `fable_5_1-v3-census`):
- `70e3a89d`: an unevaluated query now counts conservatively everywhere (`cover`, `pilot`, `summarize.py`): not
  covered, gate failed, residual fraction 1, one full piece, full projected cost. "Unevaluated" means an infinite
  domain, or a finite domain with an effective coordinate bound above 62. Before this commit such a query failed the
  gate but added 0 to residual and cost, and a non-enumerable finite domain would even have been reported as fully
  covered (new unit test).
- `f91f0137`: `census cost` also reports the plan's cost estimator (section 10).
- `4746d60c`, `7a36c606`, `f6cf69ae`, `a435e68a`: the batch now includes Q3 (with the rustred inner-pool
  environment), second hot-owner runs, a sampling-sensitivity run and provenance files.
- `e8ebd501`: the Q3 census drops the duplicated coefficient-denominator role (section 12).

**Effect of the conservative accounting [M].** Five-loop gate shares and fully-covered shares are unchanged: the first
receipt already failed unevaluated queries. Mean uncovered fractions and projected costs rise by at most the
unevaluated weight: +0.1 pp and +0.001 at gen 7, where 7 of 6000 draws (0.12%) are unevaluated.

In the four-loop controls, gates and fully-covered shares are also unchanged, but their mean uncovered fractions were
strongly understated. Unevaluated weight there is 15-70% (section 8). The corrected mean uncovered fractions are FG
95.1% (was 72.7%), BMW 28.9% (13.6%), H 97.5% (45.6%) and X 97.8% (28.0%).

## 2. Gate 0.7 and the G2' bound

T1 gives the gate shares on `hist_pps_seconds` against `natives_before_dispatch`. `hot_owner` is 011101110111000.

| run | scope | draws (distinct) | fully covered | mean uncovered fraction | unevaluated | gate D-only | gate A/R (C2) | gate hull | gate exact pointwise | proj. rel. cost binned D-only / hull / A/R [E] | proj. rel. cost OLS D-only / hull [E] |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---|---|
| gen7 wait 30 s | all_owners | 6000 (5123) | 61.6% | 3.7% | 0.12% | 70.1% | 84.8% | 89.0% | 91.9% | 0.100 / 0.057 / 0.057 | 0.163 / 0.093 |
| gen7 wait 30 s | hot_owner | 3863 (3070) | 63.8% | 2.2% | 0.00% | 72.5% | 88.7% | 92.5% | 95.4% | 0.058 / 0.025 / 0.024 | 0.127 / 0.061 |
| gen7 wait 300 s | all_owners | 6000 (5123) | 59.5% | 4.5% | 0.12% | 68.0% | 82.8% | 87.0% | 90.4% | 0.111 / 0.067 / 0.065 | 0.177 / 0.105 |
| gen7 wait 300 s | hot_owner | 3863 (3070) | 61.7% | 2.6% | 0.00% | 70.4% | 86.7% | 90.5% | 94.0% | 0.067 / 0.032 / 0.029 | 0.140 / 0.071 |
| gen7 wait 30 s, cap 2e7, 2e5 samples | all_owners | 6000 (5123) | 61.6% | 3.7% | 0.12% | 70.1% | 84.8% | 89.0% | 91.9% | 0.100 / 0.057 / 0.057 | 0.163 / 0.093 |
| gen7 wait 30 s, cap 2e7, 2e5 samples | hot_owner | 3863 (3070) | 63.8% | 2.2% | 0.00% | 72.5% | 88.7% | 92.5% | 95.4% | 0.058 / 0.025 / 0.024 | 0.127 / 0.061 |
| gen6 wait 30 s | all_owners | 6000 (5119) | 61.3% | 3.6% | 0.15% | 69.5% | 84.9% | 89.1% | 91.8% | 0.102 / 0.057 / 0.060 | 0.163 / 0.093 |
| gen6 wait 30 s | hot_owner | 3879 (3066) | 63.7% | 2.0% | 0.00% | 72.1% | 89.0% | 92.8% | 95.3% | 0.057 / 0.025 / 0.024 | 0.124 / 0.059 |
| gen3 wait 30 s | all_owners | 6000 (4262) | 55.4% | 6.4% | 0.23% | 57.4% | 72.4% | 79.9% | 84.9% | 0.169 / 0.100 / 0.096 | 0.223 / 0.137 |
| gen3 wait 30 s | hot_owner | 3641 (2175) | 56.8% | 3.8% | 0.00% | 58.3% | 76.0% | 83.9% | 89.6% | 0.122 / 0.055 / 0.050 | 0.172 / 0.086 |

Reading:
- **The gate passes by a wide margin [M]**: 70.1% with D-only cuts, against the 30% threshold. The 1-sigma binomial
  error of a 70% share over 6000 draws is about ±0.6 pp [E]. The draws are with replacement and cover 5,123 distinct
  natives.
- **The dispatch margin is a proxy.** Dispatch time is reconstructed as commit time minus native seconds minus a wait.
  Raising the wait from 30 s to 300 s lowers the D-only gate from 70.1% to 68.0% [M].
- **Sampling precision (item 6) [M].** At gen 7, 103 of 6000 draws (1.7%) were decided on 20,000 sampled points
  because Q had more than 2e6 points. Re-running with exact enumeration up to 2e7 points and 200,000 samples above
  (`gen7/cover-w30-cap2e7.json`) leaves 64 sampled draws. Every share in T1 is unchanged to 0.1 pp. Exactly one more
  draw becomes partially uncovered (2,298 to 2,299 draws with an uncovered point), so "fully covered" was overestimated
  by 1 draw in 6000 (0.02 pp). Piece counts move slightly (section 4).
- **Stable across generations [M].** Gen 6 matches gen 7 (69.5% D-only). Gen 3 is lower (57.4%): early in the run
  fewer merged natives exist.

## 3. Anchor ladder, weighting, G2' at resume, general union cover (D2)

| sample | anchors | scope | draws | fully covered | mean uncovered | gate D-only | gate A/R | gate hull | proj. rel. cost binned D-only / hull [E] |
|---|---|---|---:|---:|---:|---:|---:|---:|---|
| hist_pps_seconds | all_earlier_ids | all_owners | 6000 | 76.7% | 1.7% | 83.1% | 93.2% | 95.9% | 0.049 / 0.024 |
| hist_pps_seconds | natives_before_commit | all_owners | 6000 | 66.1% | 2.5% | 74.3% | 88.3% | 93.0% | 0.077 / 0.035 |
| hist_pps_seconds | natives_before_dispatch | all_owners | 6000 | 61.6% | 3.7% | 70.1% | 84.8% | 89.0% | 0.100 / 0.057 |
| hist_pps_seconds | all_earlier_ids | hot_owner | 3863 | 78.8% | 0.8% | 85.0% | 96.0% | 97.8% | 0.024 / 0.007 |
| hist_pps_seconds | natives_before_commit | hot_owner | 3863 | 67.7% | 1.5% | 75.8% | 91.1% | 95.3% | 0.046 / 0.015 |
| hist_pps_seconds | natives_before_dispatch | hot_owner | 3863 | 63.8% | 2.2% | 72.5% | 88.7% | 92.5% | 0.058 / 0.025 |
| hist_uniform | all_earlier_ids | all_owners | 6000 | 67.2% | 6.0% | 69.6% | 77.3% | 81.9% | 0.198 / 0.154 |
| hist_uniform | natives_before_commit | all_owners | 6000 | 62.4% | 7.1% | 65.3% | 73.2% | 78.2% | 0.228 / 0.177 |
| hist_uniform | natives_before_dispatch | all_owners | 6000 | 57.1% | 8.7% | 59.8% | 68.6% | 72.7% | 0.264 / 0.211 |
| hist_uniform | all_earlier_ids | hot_owner | 305 | 76.1% | 3.7% | 79.7% | 85.9% | 88.2% | 0.106 / 0.070 |
| hist_uniform | natives_before_commit | hot_owner | 305 | 67.9% | 5.1% | 72.8% | 80.0% | 83.3% | 0.144 / 0.093 |
| hist_uniform | natives_before_dispatch | hot_owner | 305 | 64.6% | 6.2% | 69.5% | 77.4% | 80.3% | 0.166 / 0.113 |
| pending_pps_predicted | all_natives | all_owners | 6000 | 55.4% | 3.7% | 63.8% | 81.3% | 85.7% | 0.129 / 0.070 |
| pending_pps_predicted | all_other_domains | all_owners | 6000 | 79.5% | 1.1% | 85.0% | 94.4% | 96.4% | 0.045 / 0.020 |
| pending_pps_predicted | all_natives | hot_owner | 3253 | 62.0% | 2.3% | 70.9% | 89.6% | 92.6% | 0.067 / 0.032 |
| pending_pps_predicted | all_other_domains | hot_owner | 3253 | 83.1% | 0.7% | 88.4% | 97.5% | 98.2% | 0.021 / 0.009 |
| pending_uniform | all_natives | all_owners | 6000 | 56.7% | 7.8% | 61.1% | 70.9% | 76.2% | 0.249 / 0.199 |
| pending_uniform | all_other_domains | all_owners | 6000 | 80.9% | 3.0% | 83.7% | 88.9% | 91.7% | 0.107 / 0.085 |
| pending_uniform | all_natives | hot_owner | 320 | 65.6% | 5.4% | 68.8% | 75.9% | 80.0% | 0.149 / 0.103 |
| pending_uniform | all_other_domains | hot_owner | 320 | 89.7% | 1.1% | 91.2% | 94.7% | 96.2% | 0.041 / 0.024 |

population: 5949328 Apply natives, 241443 s; finite pending Apply 5847414, predicted 115809 s [E]

Reading:
- **Anchor ladder at gen 7, PPS, all owners [M]:**

  | anchor set | fully covered | D-only gate | hull gate | projected D-only cost [E] |
  |---|---:|---:|---:|---:|
  | natives before dispatch (G2') | 61.6% | 70.1% | 89.0% | 0.100 |
  | natives before commit | 66.1% | 74.3% | 93.0% | 0.077 |
  | all earlier IDs (general union cover, D2) | 76.7% | 83.1% | 95.9% | 0.049 |

  D2 beyond G2' adds 15 pp of fully covered Apply CPU. The D2 bound needs covers that may be pending, retired or
  aliased (cycles possible). It is an upper bound, not an implementable rule.
- **Per-inspection weighting is lower [M]:** fully covered 57.1%, D-only gate 59.8%, hull 72.7%. Cheap natives are
  covered less often than expensive ones.
- **G2' at resume [M]** (pending Apply at gen 7 against all gen-7 natives, `pending_pps_predicted`): fully covered
  55.4%, D-only gate 63.8%, A/R 81.3%, hull 85.7%, projected D-only cost 0.129 [E]. Per pending domain
  (`pending_uniform`) the figures are 56.7% / 61.1% / 70.9% / 76.2%.
- **D2 at resume** (`all_other_domains`): fully covered 79.5%, D-only 85.0%, hull 96.4% [M]. This is relevant only if
  the ultimate campaign imported v2 state. The owner prefers a fresh campaign, for which the historical
  (`hist_*`) rows are the relevant bound.
- The pending samples exclude the 31 infinite pending Apply domains, which have no finite point count or predicted
  cost. The finite pending Apply population is 5,847,414 domains with 115,809 predicted seconds [E].

## 4. Residual piece counts: D-only vs A/R cuts

Draws are counted with their PPS multiplicity. "Partial" means Q has at least one uncovered point.

| run / sample / anchors | draws | partial (uncovered > 0) | sampled (inexact) | unevaluated | D-only pieces | A/R pieces |
|---|---:|---:|---:|---:|---|---|
| gen7 cover-w30 / hist_pps_seconds / natives_before_dispatch | 6000 | 2298 | 103 | 7 | 1: 2284, 2: 14 | 1: 1966, 2: 292, 3: 29, 4-8: 7, >8: 4 (max 19) |
| gen7 cover-w30 / hist_uniform / natives_before_dispatch | 6000 | 2571 | 20 | 0 | 1: 2570, 2: 1 | 1: 2489, 2: 81, 3: 1 |
| gen7 cover-w30 / pending_pps_predicted / all_natives | 6000 | 2678 | 98 | 0 | 1: 2673, 2: 4, 3: 1 | 1: 2445, 2: 225, 3: 8 |
| gen7 cover-w30 / pending_pps_predicted / all_other_domains | 6000 | 1231 | 98 | 0 | 1: 1230, 2: 1 | 1: 1210, 2: 21 |
| gen7 cover-w30-cap2e7 / hist_pps_seconds / natives_before_dispatch | 6000 | 2299 | 64 | 7 | 1: 2293, 2: 6 | 1: 1979, 2: 286, 3: 27, 4-8: 3, >8: 4 (max 19) |
| gen6 cover-w30 / hist_pps_seconds / natives_before_dispatch | 6000 | 2311 | 116 | 9 | 1: 2296, 2: 13, 3: 2 | 1: 1974, 2: 296, 3: 31, 4-8: 7, >8: 3 (max 19) |
| gen3 cover-w30 / hist_pps_seconds / natives_before_dispatch | 6000 | 2665 | 114 | 14 | 1: 2651, 2: 13, 3: 1 | 1: 2143, 2: 461, 3: 37, 4-8: 18, >8: 6 (max 19) |

Reading [M]:
- **D-only residuals are almost always one piece:** 2,284 of 2,298 partial draws at gen 7, and never more than 3
  pieces.
- **A/R rectangles fragment more.** At gen 7, 14% of partial draws need 2 or more rectangles, and 4 draws need more
  than 8 (up to 19).
- **Fragmentation risk R9 is negligible for D-only and hull residuals, with a small tail for A/R.** This is a further
  argument to start W4.2 with D-only or hull residuals.
- Sampling changes the D-only 2-piece count (14 to 6 at 10x precision), because sampled points define the bands. The
  1-piece majority is unaffected.

## 5. Per owner at gen 7

Top 10 owners by share of `hist_pps_seconds` draws, against `natives_before_dispatch`:

| owner | share of draws | fully covered | gate D-only | gate A/R | gate hull | mean uncovered | unevaluated |
|---|---:|---:|---:|---:|---:|---:|---:|
| 011101110111000 | 64.4% | 63.8% | 72.5% | 88.7% | 92.5% | 2.2% | 0.0% |
| 010011111101011 | 5.5% | 49.4% | 51.2% | 72.4% | 80.9% | 6.6% | 0.0% |
| 111001100111001 | 4.5% | 75.8% | 82.4% | 92.7% | 94.9% | 2.1% | 0.0% |
| 110110111101001 | 3.7% | 80.5% | 82.7% | 90.0% | 92.3% | 1.8% | 0.0% |
| 110001011101011 | 2.6% | 66.7% | 76.3% | 89.7% | 92.3% | 4.2% | 0.0% |
| 000011001001011 | 1.7% | 14.7% | 17.6% | 27.5% | 34.3% | 25.5% | 6.9% |
| 111001100101101 | 1.7% | 45.5% | 67.3% | 76.2% | 85.1% | 6.1% | 0.0% |
| 111000010101001 | 1.7% | 68.7% | 74.7% | 80.8% | 87.9% | 3.3% | 0.0% |
| 111000100111001 | 1.1% | 62.7% | 65.7% | 74.6% | 79.1% | 4.9% | 0.0% |
| 001001100101111 | 1.1% | 51.5% | 78.8% | 89.4% | 90.9% | 4.3% | 0.0% |

Reading:
- The v2 hot owner 011101110111000 carries 64.4% of the draws (64.1% of Apply seconds, section 10) and passes best:
  72.5% D-only, 92.5% hull [M].
- Owner 000011001001011 is the worst large owner at gen 7: 17.6% D-only, 34.3% hull, and 6.9% of its draws are
  unevaluated (infinite). It is also the pilot's CPU-hot owner (section 6).

## 6. The pilot's hot owner is 000011001001011 (item 4)

**Finding [M].** Only 53 of the 4000 PPS draws (1.3%) land on 011101110111000 in the pilot. The first receipt had 53
and 59 draws in its two runs. This is not a mask error:
- Owner strings are parsed identically for the pilot's `result.json` and for CP5, as `owner_string` of the bits
  `1 << i`.
- The pilot and v2 use the same selection manifest (sha256 d2667dc9...).

The pilot was **seeded** with three queries in 011101110111000 at A <= 13, R <= 3. Those domains are tiny: 9,852
natives with a mean of 2.3 points in the smallest decade, 1.2% of the pilot's Apply seconds (`pilot/cost.json`).
The closure they generate spends its CPU in other owners:
- **000011001001011: 37.4% of Apply seconds, 38.0% of PPS draws**;
- 010111011000001: 16.7% / 16.1%;
- 101010000110001: 7.6% / 8.2%.

This agrees with the review lens (`scale_owner.txt`: 3,836 of 10,225 s for 000011001001011). C-5F has the same
profile: 000011001001011 holds 32.1% of draws and 31.0% of Apply seconds, while 011101110111000 holds 38 of 4000
draws (T4c, T7).

**Corrected pilot rows [M].** The pilot's `hot_owner` block in `pilot/pilot-cover.json` describes a 1.3% owner and
must not be read as the hot owner. The rows for the pilot's CPU-hot owner come from `pilot/pilot-cover-hot2.json`
(`--hot 000011001001011`). The all-owner rows are unchanged.

Pilot per owner (hist_pps_seconds / natives_before_dispatch):

| owner | share of draws | fully covered | gate D-only | gate A/R | gate hull | mean uncovered | unevaluated |
|---|---:|---:|---:|---:|---:|---:|---:|
| 000011001001011 | 38.0% | 50.3% | 61.2% | 75.1% | 77.8% | 9.6% | 0.0% |
| 010111011000001 | 16.1% | 68.9% | 75.4% | 81.0% | 85.1% | 4.9% | 0.0% |
| 101010000110001 | 8.2% | 64.5% | 76.5% | 82.0% | 85.6% | 5.1% | 0.0% |
| 001100101110000 | 6.3% | 49.2% | 71.7% | 83.5% | 87.4% | 4.5% | 0.0% |
| 011001000001111 | 5.8% | 51.5% | 67.4% | 73.0% | 81.1% | 6.8% | 0.0% |
| 000010011001001 | 5.6% | 31.2% | 62.9% | 74.6% | 80.4% | 6.3% | 0.0% |
| 111000010101001 | 5.1% | 62.4% | 64.4% | 67.8% | 70.2% | 13.3% | 0.0% |
| 111000100011101 | 4.8% | 62.2% | 67.4% | 69.9% | 71.5% | 15.6% | 0.0% |
| 001000100101111 | 4.8% | 45.8% | 62.1% | 67.9% | 74.2% | 10.7% | 0.0% |
| 111001000001111 | 1.8% | 51.4% | 63.5% | 66.2% | 77.0% | 11.7% | 0.0% |

Pilot and controls, all owners and hot-owner rows:

| run | scope (hot mask) | anchors | draws | fully covered | mean uncovered | unevaluated | gate D-only | gate A/R | gate hull | proj. rel. cost binned D-only / hull [E] |
|---|---|---|---:|---:|---:|---:|---:|---:|---:|---|
| pilot | all | all_earlier_ids | 4000 | 68.4% | 3.7% | 0.0% | 78.3% | 85.7% | 88.4% | 0.076 / 0.048 |
| pilot | all | natives_before_commit | 4000 | 56.0% | 5.7% | 0.0% | 69.0% | 79.2% | 83.0% | 0.114 / 0.072 |
| pilot | all | natives_before_dispatch | 4000 | 53.6% | 8.8% | 0.0% | 66.0% | 75.2% | 79.2% | 0.152 / 0.113 |
| pilot | 011101110111000 | all_earlier_ids | 53 | 66.0% | 22.0% | 0.0% | 66.0% | 66.0% | 66.0% | 0.239 / 0.232 |
| pilot | 011101110111000 | natives_before_commit | 53 | 52.8% | 25.6% | 0.0% | 52.8% | 52.8% | 52.8% | 0.280 / 0.273 |
| pilot | 011101110111000 | natives_before_dispatch | 53 | 41.5% | 41.7% | 0.0% | 41.5% | 41.5% | 41.5% | 0.463 / 0.462 |
| pilot | 000011001001011 | all_earlier_ids | 1521 | 65.7% | 3.5% | 0.0% | 75.3% | 85.1% | 87.2% | 0.048 / 0.023 |
| pilot | 000011001001011 | natives_before_commit | 1521 | 51.7% | 6.1% | 0.0% | 63.2% | 78.1% | 81.0% | 0.083 / 0.043 |
| pilot | 000011001001011 | natives_before_dispatch | 1521 | 50.3% | 9.6% | 0.0% | 61.2% | 75.1% | 77.8% | 0.125 / 0.091 |
| C-5F | all | all_earlier_ids | 4000 | 65.4% | 16.8% | 0.0% | 65.4% | 65.8% | 66.3% | 0.269 / 0.205 |
| C-5F | all | natives_before_commit | 4000 | 48.5% | 25.3% | 0.0% | 48.6% | 49.0% | 49.7% | 0.403 / 0.306 |
| C-5F | all | natives_before_dispatch | 4000 | 44.2% | 29.1% | 0.0% | 44.4% | 44.9% | 45.4% | 0.440 / 0.346 |
| C-5F | 011101110111000 | all_earlier_ids | 38 | 0.0% | 86.8% | 0.0% | 0.0% | 0.0% | 0.0% | 0.992 / 0.992 |
| C-5F | 011101110111000 | natives_before_commit | 38 | 0.0% | 87.9% | 0.0% | 0.0% | 0.0% | 0.0% | 0.993 / 0.993 |
| C-5F | 011101110111000 | natives_before_dispatch | 38 | 0.0% | 100.0% | 0.0% | 0.0% | 0.0% | 0.0% | 1.000 / 1.000 |
| C-5F | 000011001001011 | all_earlier_ids | 1284 | 72.2% | 11.3% | 0.0% | 72.2% | 72.9% | 73.1% | 0.194 / 0.141 |
| C-5F | 000011001001011 | natives_before_commit | 1284 | 54.5% | 20.1% | 0.0% | 54.5% | 54.6% | 54.8% | 0.326 / 0.241 |
| C-5F | 000011001001011 | natives_before_dispatch | 1284 | 49.6% | 22.7% | 0.0% | 49.6% | 50.2% | 50.2% | 0.360 / 0.275 |
| four-loop FG | all | all_earlier_ids | 4000 | 0.1% | 94.9% | 22.4% | 0.9% | 1.0% | 1.0% | 0.987 / 0.987 |
| four-loop FG | all | natives_before_commit | 4000 | 0.1% | 94.9% | 22.4% | 0.9% | 1.0% | 1.0% | 0.987 / 0.987 |
| four-loop FG | all | natives_before_dispatch | 4000 | 0.0% | 95.1% | 22.4% | 0.7% | 0.9% | 0.9% | 0.988 / 0.988 |
| four-loop BMW | all | all_earlier_ids | 4000 | 26.5% | 27.6% | 15.2% | 47.6% | 70.4% | 70.4% | 0.632 / 0.619 |
| four-loop BMW | all | natives_before_commit | 4000 | 23.8% | 27.8% | 15.2% | 45.1% | 70.3% | 70.3% | 0.654 / 0.641 |
| four-loop BMW | all | natives_before_dispatch | 4000 | 21.0% | 28.9% | 15.2% | 40.5% | 67.7% | 67.7% | 0.678 / 0.663 |
| four-loop H | all | all_earlier_ids | 4000 | 0.2% | 97.4% | 51.8% | 1.1% | 1.1% | 1.2% | 0.997 / 0.997 |
| four-loop H | all | natives_before_commit | 4000 | 0.1% | 97.4% | 51.8% | 1.1% | 1.1% | 1.2% | 0.997 / 0.997 |
| four-loop H | all | natives_before_dispatch | 4000 | 0.0% | 97.5% | 51.8% | 0.9% | 1.0% | 1.0% | 0.999 / 0.999 |
| four-loop X | all | all_earlier_ids | 4000 | 0.6% | 97.7% | 69.8% | 1.6% | 1.6% | 1.6% | 0.993 / 0.993 |
| four-loop X | all | natives_before_commit | 4000 | 0.2% | 97.8% | 69.8% | 1.4% | 1.4% | 1.4% | 0.997 / 0.997 |
| four-loop X | all | natives_before_dispatch | 4000 | 0.2% | 97.8% | 69.8% | 1.4% | 1.4% | 1.4% | 0.997 / 0.997 |

Reading:
- **Pilot, all owners (G2' anchors) [M]:** fully covered 53.6%, D-only gate 66.0%, A/R 75.2%, hull 79.2%.
- **Pilot, CPU-hot owner 000011001001011 [M]:** fully covered 50.3%, D-only gate 61.2%, A/R 75.1%, hull 77.8%,
  projected cost 0.125 / 0.091 [E].
- **Cross-check with the plan's C-HOT bound [M].** The plan measured 69.3% of Apply seconds fully covered by all
  earlier IDs; the census gives 68.4% on the pilot (`all_earlier_ids`).

## 7. Hot-owner closure import (W0.11 input): about 0 benefit

This asks whether importing the pilot's natives as anchors would discharge v2 work. The pilot's natives serve as
anchors for v2 gen-7 Apply domains in the same buckets (801 pilot buckets; the v2 natives in those buckets hold
169,606 s, 70.2% of v2's Apply seconds).

`pilot/v2g7-vs-pilot.json`: pilot records 7767543, pilot buckets 801; v2 Apply natives in pilot buckets [879860, 169605.66776641496]; v2 Apply pending in pilot buckets [841457, 77768.11411316568] [pred. s E]; pilot Apply natives [2372220, 10232.33250848831]

| sample | scope | draws | fully covered | gate D-only | gate hull | mean uncovered | proj. rel. cost hull [E] | unevaluated |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| pilot_apply_natives_pps_seconds_vs_v2 | all | 4000 | 100.0% | 100.0% | 100.0% | 0.0% | 0.000 | 0.0% |
| pilot_apply_natives_pps_seconds_vs_v2 | hot | 59 | 100.0% | 100.0% | 100.0% | 0.0% | 0.000 | 0.0% |
| pilot_apply_natives_pps_seconds_vs_v2 | other | 3941 | 100.0% | 100.0% | 100.0% | 0.0% | 0.000 | 0.0% |
| v2_apply_natives_pps_seconds | all | 4000 | 0.4% | 0.5% | 1.1% | 72.3% | 0.866 | 0.0% |
| v2_apply_natives_pps_seconds | hot | 3657 | 0.4% | 0.5% | 1.1% | 71.8% | 0.861 | 0.0% |
| v2_apply_natives_pps_seconds | other | 343 | 0.6% | 0.6% | 1.5% | 77.8% | 0.923 | 0.0% |
| v2_apply_pending_pps_predicted | all | 4000 | 0.0% | 0.1% | 0.2% | 81.8% | 0.928 | 0.0% |
| v2_apply_pending_pps_predicted | hot | 3225 | 0.0% | 0.1% | 0.1% | 80.7% | 0.917 | 0.0% |
| v2_apply_pending_pps_predicted | other | 775 | 0.0% | 0.4% | 0.8% | 86.2% | 0.973 | 0.0% |
| v2_apply_pending_uniform | all | 4000 | 2.2% | 3.5% | 4.8% | 64.7% | 0.860 | 0.0% |
| v2_apply_pending_uniform | hot | 1525 | 0.4% | 0.6% | 0.8% | 73.3% | 0.877 | 0.0% |
| v2_apply_pending_uniform | other | 2475 | 3.3% | 5.3% | 7.2% | 59.4% | 0.849 | 0.0% |

exact identical domains [in pilot buckets, identical, total]: delegated|Apply|hot [580032, 1785, 580032]; delegated|Apply|other [661454, 4385, 6989261]; delegated|Route|other [2883283, 79037, 18549239]; native|Apply|hot [273737, 862, 273737]; native|Apply|other [606581, 3837, 5675591]; native|Route|other [2530753, 51478, 21516094]; pending|Apply|hot [317309, 0, 317309]; pending|Apply|other [524179, 776, 5530136]; pending|Route|other [1949339, 1233, 14724634]

`pilot/v2g7-vs-pilot-hot2.json`: pilot records 7767543, pilot buckets 801; v2 Apply natives in pilot buckets [879860, 169605.66776641496]; v2 Apply pending in pilot buckets [841457, 77768.11411316568] [pred. s E]; pilot Apply natives [2372220, 10232.33250848831]

| sample | scope | draws | fully covered | gate D-only | gate hull | mean uncovered | proj. rel. cost hull [E] | unevaluated |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| pilot_apply_natives_pps_seconds_vs_v2 | all | 4000 | 100.0% | 100.0% | 100.0% | 0.0% | 0.000 | 0.0% |
| pilot_apply_natives_pps_seconds_vs_v2 | hot | 1506 | 100.0% | 100.0% | 100.0% | 0.0% | 0.000 | 0.0% |
| pilot_apply_natives_pps_seconds_vs_v2 | other | 2494 | 100.0% | 100.0% | 100.0% | 0.0% | 0.000 | 0.0% |
| v2_apply_natives_pps_seconds | all | 4000 | 0.4% | 0.5% | 1.1% | 72.3% | 0.866 | 0.0% |
| v2_apply_natives_pps_seconds | hot | 82 | 0.0% | 0.0% | 0.0% | 83.6% | 0.969 | 0.0% |
| v2_apply_natives_pps_seconds | other | 3918 | 0.5% | 0.6% | 1.2% | 72.1% | 0.864 | 0.0% |
| v2_apply_pending_pps_predicted | all | 4000 | 0.0% | 0.1% | 0.2% | 81.8% | 0.928 | 0.0% |
| v2_apply_pending_pps_predicted | hot | 193 | 0.0% | 0.0% | 0.0% | 85.5% | 0.993 | 0.0% |
| v2_apply_pending_pps_predicted | other | 3807 | 0.0% | 0.2% | 0.3% | 81.6% | 0.925 | 0.0% |
| v2_apply_pending_uniform | all | 4000 | 2.2% | 3.5% | 4.8% | 64.7% | 0.860 | 0.0% |
| v2_apply_pending_uniform | hot | 31 | 0.0% | 0.0% | 0.0% | 63.7% | 0.895 | 0.0% |
| v2_apply_pending_uniform | other | 3969 | 2.2% | 3.6% | 4.8% | 64.7% | 0.859 | 0.0% |

exact identical domains [in pilot buckets, identical, total]: delegated|Apply|hot [13038, 0, 13038]; delegated|Apply|other [1228448, 6170, 7556255]; delegated|Route|other [2883283, 79037, 18549239]; native|Apply|hot [8211, 0, 8211]; native|Apply|other [872107, 4699, 5941117]; native|Route|other [2530753, 51478, 21516094]; pending|Apply|hot [6871, 0, 6871]; pending|Apply|other [834617, 776, 5840574]; pending|Route|other [1949339, 1233, 14724634]

Reading [M]:
- **The pilot's natives discharge almost none of v2's work:**
  - 0.4% of v2's natives in those buckets are fully covered (PPS), with a mean uncovered fraction of 72.3% and a
    projected hull cost of 0.866 [E];
  - 0.0% of v2's pending Apply in those buckets is fully covered (77,768 predicted s [E]); 2.2% per pending domain.
- **This holds for both hot owners:**
  - 011101110111000: 0.4% fully covered;
  - 000011001001011: 0.0% fully covered (82 draws), and 0 of v2's 8,211 natives in that owner are identical to a
    pilot domain.
- **In the other direction, v2 covers 100% of the pilot's natives.** The pilot's closure is a small corner of v2's
  point space, so a closure import is not worth building.

## 8. Controls: C-5F and four-loop

See T5 for the rows.
- **C-5F [M]:** fully covered 44.2%, D-only 44.4%, hull 45.4% against merged natives. Coverage is all-or-nothing:
  partial domains with at most 10% residual are rare. Its CPU-hot owner 000011001001011 is at 49.6% / 49.6% / 50.2%.
- **Four-loop FG, H, X [M]:**
  - unevaluated (infinite) weight is 22.4%, 51.8% and 69.8% of Apply CPU;
  - fully covered is at most 0.6% and the gates at most 1.6%;
  - mean uncovered is at least 95%, because the infinite queries count as fully uncovered.
- **Four-loop BMW [M]:** 15.2% unevaluated, fully covered 21.0%, D-only 40.5%, hull 67.7%.
- **Consequence [E]:** the four-loop per-family controls cannot show a G2' benefit except on BMW. The W4.2 gate must be
  judged on C-5F and C-HOT-sub. The four-loop runs remain the soundness controls.

## 9. Composition of the native-pending, and pending vs committed envelope (gen 7)

| slice (Apply unless noted) | domains | points | infinite | predicted Apply s [E] |
|---|---:|---:|---:|---:|
| unreserved / reserved / started | 5,847,408 / 6 / 31 | 8.124e11 | 31 | 115,809 |
| admitted in g5 / g6 / g7 | 2,427,672 / 2,230,438 / 1,189,335 | 1.46e11 / 6.66e11 / 6.69e8 | 0 / 31 / 0 | 47,052 / 51,752 / 17,005 |
| position past the ledger cursor: < 1e6 / 1e6-1e7 / >= 1e7 IDs | 139,266 / 1,779,578 / 3,928,601 | 1.17e8 / 1.46e11 / 6.66e11 | 0 / 0 / 31 | 2,017 / 42,639 / 71,153 |
| t = 9 active coordinates | 2,207,401 | 3.34e9 | 0 | 81,752 (70.6%) |
| owner 011101110111000 | 317,309 | 1.66e8 | 0 | 62,254 (53.8%) |
| owner 111001100111001 | 785,629 | 5.52e8 | 0 | 8,570 (7.4%) |
| owner 000011001001011 | 6,871 | 8.32e7 | 0 | 4,095 (3.5%) |
| Route (all states) | 14,724,634 | 4.290e12 | 36 | - |

Pending vs committed envelope (Apply): pending points outside the committed per-owner (A, R) level set 6.65e7 of
8.12e11; outside the committed staircase 4.76e7; pending domains touching them 7,983 and 7,725 of 5,847,414.
Pending domains beyond the committed extrema (Amax, Rmax, Pmax, Dmin): Apply 5,304 / 66 / 77 / 11 of 5,847,445;
Route 40,655 / 16,789 / 5,183 / 9,347 of 14,724,634. Full tables: `receipt-v4/gen7/compose.json`, `w0_tables.py` T8.

Reading:
- **Native-pending [M]:** 20,572,079 domains. Apply: 5,847,445 (31 infinite), of which 5,847,408 are unreserved.
  Route: 14,724,634 (36 infinite).
- **By admission generation, Apply [M]:** 2.43M (g5), 2.23M (g6), 1.19M (g7).
- **Queue position [M]:** 3.93M Apply pending sit at least 1e7 IDs past the ledger cursor.
- **Predicted Apply pending CPU [E], binned law:** 115,809 s, against 241,443 s committed.
  - 53.8% (62,254 s) is in the hot owner's 317,309 domains;
  - 70.6% is at t = 9 active coordinates.
- **Escape shells are negligible [M].** 6.65e7 of 8.12e11 pending Apply points (8.2e-5) lie outside the committed
  per-owner (A, R) level set, and only 7,983 of 5.85M pending Apply domains touch it. Against the committed
  extrema, 5,304 Apply pending domains exceed Amax, 66 exceed Rmax, 77 exceed Pmax and 11 fall below Dmin.
- **Consequence:** the plan's condition for P-anchors ("if 0.7 shows escape shells dominating", W4.4) is not met.

## 10. Cost laws, and why the census exponents (0.51-0.66) differ from the plan's points^0.74-0.90 (item 5)

**The two figures use different estimators on different data [M].**
- **The plan's figure.** "Cost ∝ points^0.74-0.90" (plan section 1) came from `perfskeptic/slopes.py` of the review
  session, applied to `scale_owner.txt` and `scale_v2g6.txt`.
  - Estimator: an n-weighted least-squares fit of **ln(mean seconds) on ln(mean points) over decade bins**
    (floor(log10 points), bins with at least 10 natives).
  - Data: the drained C-HOT pilot (`scale_owner.txt`, 7,767,543 records) and **the gen-6 record segment only** of v2
    (`scale_v2g6.txt`, 7,215,231 records, which is the line count of `records-00000000000000000006.jsonl`).
  - Re-running `slopes.py` on those files reproduces the plan: the pilot's owners give 0.74-0.90 (000011001001011
    0.90); v2's gen-6 hot owner gives 0.74; other v2 owners give 0.38-0.85.
- **The census's figure.** The census "cost exponent" is the **per-native OLS of ln(seconds) on ln(points)**, a fit of
  the conditional mean of ln(seconds), i.e. a geometric-mean law. It covers all natives of an owner in the
  checkpoint; at gen 7 that is cumulative over generations 1-7.
- **Why they differ.** Seconds are right-skewed and their spread grows with size, so ln(mean seconds) rises faster
  than mean ln(seconds). On the same data the bin-mean slope therefore exceeds the OLS slope.
- `census cost` now computes both estimators. `binmean_wls` reproduces `slopes.py` and is unit-tested on an exact
  power law.

`gen7/cost.json`: Apply seconds 241443; pooled OLS b 0.351 (r2 0.34, n 5948867); pooled bin-mean WLS b 0.567

| owner | Apply s share | natives | OLS b (r2) | bin-mean WLS b | OLS b < 1e5 pts | bin-mean b < 1e5 pts | top-decade b | max decade | OLS b by gen | bin-mean b by gen |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---|---|
| 011101110111000 | 64.1% | 273737 | 0.66 (0.38) | 0.75 | 0.66 | 0.75 | 1.37 | 4 | g3 0.77, g4 0.68, g5 0.58, g6 0.61, g7 0.67 | g3 0.79, g4 0.79, g5 0.64, g6 0.74, g7 0.79 |
| 010011111101011 | 5.2% | 294212 | 0.56 (0.36) | 0.68 | 0.56 | 0.68 | 1.08 | 5 | g3 0.57, g4 0.55, g5 0.57, g6 0.54 | g3 0.65, g4 0.68, g5 0.75, g6 0.65 |
| 111001100111001 | 4.8% | 606499 | 0.53 (0.54) | 0.59 | 0.53 | 0.59 | 0.99 | 5 | g3 0.53, g4 0.53, g5 0.49, g6 0.59, g7 0.61 | g3 0.59, g4 0.58, g5 0.53, g6 0.65, g7 0.67 |
| 110110111101001 | 3.9% | 642789 | 0.58 (0.52) | 0.66 | 0.58 | 0.66 | 1.09 | 5 | g3 0.57, g4 0.57, g5 0.62, g6 0.61, g7 0.61 | g3 0.64, g4 0.63, g5 0.70, g6 0.67, g7 0.68 |
| 110001011101011 | 2.4% | 142401 | 0.63 (0.65) | 0.73 | 0.63 | 0.73 | 0.74 | 5 | g3 0.60, g4 0.61, g5 0.63, g6 0.67, g7 0.73 | g3 0.69, g4 0.71, g5 0.71, g6 0.77, g7 0.84 |
| 111000010101001 | 1.8% | 212924 | 0.51 (0.46) | 0.72 | 0.49 | 0.71 | 0.64 | 6 | g3 0.52, g4 0.51, g5 0.49, g6 0.54, g7 3.52 | g3 0.77, g4 0.73, g5 0.69, g6 0.73, g7 4.56 |
| 000011001001011 | 1.6% | 8211 | 0.55 (0.45) | 0.86 | 0.32 | 0.68 | 1.53 | 5 | g3 0.59, g4 0.47, g5 0.49, g6 0.67 | g3 0.94, g4 0.70, g5 0.71, g6 0.84 |
| 111001100101101 | 1.5% | 265006 | 0.59 (0.63) | 0.69 | 0.59 | 0.69 | 0.97 | 6 | g3 0.54, g4 0.59, g5 0.60, g6 0.61, g7 0.72 | g3 0.64, g4 0.69, g5 0.69, g6 0.71, g7 0.82 |

`pilot/cost.json`: Apply seconds 10232; pooled OLS b 0.613 (r2 0.56, n 2372220); pooled bin-mean WLS b 0.710

| owner | Apply s share | natives | OLS b (r2) | bin-mean WLS b | OLS b < 1e5 pts | bin-mean b < 1e5 pts | top-decade b | max decade | OLS b by gen | bin-mean b by gen |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---|---|
| 000011001001011 | 37.4% | 382895 | 0.69 (0.45) | 0.91 | 0.69 | 0.91 | 1.71 | 4 | g0 0.69 | g0 0.91 |
| 010111011000001 | 16.7% | 439425 | 0.67 (0.46) | 0.83 | 0.67 | 0.83 | 0.94 | 4 | g0 0.67 | g0 0.83 |
| 101010000110001 | 7.6% | 152590 | 0.67 (0.74) | 0.76 | 0.67 | 0.76 | 0.72 | 4 | g0 0.67 | g0 0.76 |
| 011001000001111 | 5.7% | 220589 | 0.68 (0.74) | 0.74 | 0.68 | 0.74 | 0.52 | 4 | g0 0.68 | g0 0.74 |
| 001100101110000 | 5.7% | 88055 | 0.66 (0.74) | 0.74 | 0.66 | 0.74 | 0.68 | 5 | g0 0.66 | g0 0.74 |
| 000010011001001 | 5.6% | 39960 | 0.72 (0.86) | 0.77 | 0.72 | 0.77 | 0.53 | 5 | g0 0.72 | g0 0.77 |
| 001000100101111 | 5.4% | 140215 | 0.71 (0.64) | 0.78 | 0.71 | 0.78 | 0.97 | 4 | g0 0.71 | g0 0.78 |
| 111000010101001 | 5.4% | 525070 | 0.59 (0.47) | 0.74 | 0.59 | 0.74 | 0.79 | 3 | g0 0.59 | g0 0.74 |
| 111000100011101 | 5.1% | 268729 | 0.61 (0.42) | 0.71 | 0.61 | 0.71 | 1.21 | 4 | g0 0.61 | g0 0.71 |
| 111001000001111 | 2.1% | 70199 | 0.76 (0.72) | 0.80 | 0.76 | 0.80 | 0.95 | 4 | g0 0.76 | g0 0.80 |

`c5f/cost.json`: Apply seconds 826; pooled OLS b 0.663 (r2 0.57, n 275157); pooled bin-mean WLS b 0.668

| owner | Apply s share | natives | OLS b (r2) | bin-mean WLS b | OLS b < 1e5 pts | bin-mean b < 1e5 pts | top-decade b | max decade | OLS b by gen | bin-mean b by gen |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---|---|
| 000011001001011 | 31.0% | 58986 | 0.73 (0.41) | 0.87 | 0.73 | 0.87 | 1.02 | 3 | g3 0.73 | g3 0.87 |
| 010111011000001 | 15.1% | 46586 | 0.76 (0.50) | 0.79 | 0.76 | 0.79 | 0.85 | 3 | g3 0.76 | g3 0.79 |
| 000010011001001 | 10.0% | 12774 | 0.65 (0.76) | 0.61 | 0.65 | 0.61 | 0.84 | 4 | g3 0.65 | g3 0.61 |
| 101010000110001 | 9.1% | 28247 | 0.66 (0.76) | 0.71 | 0.66 | 0.71 | 0.72 | 3 | g3 0.66 | g3 0.71 |
| 111000010101001 | 7.9% | 43992 | 0.77 (0.54) | 0.69 | 0.77 | 0.69 | 0.67 | 2 | g3 0.77 | g3 0.69 |

Reading [M]:
- **Same data, both estimators.** At gen 7 the large owners' OLS exponents (0.51-0.66) become 0.59-0.86 with the
  plan's estimator; the pooled exponent goes from 0.351 to 0.567. For the v2 hot owner:
  - on the gen-6 record segment, the plan's estimator gives **0.74, exactly the plan's value**, while OLS on the same
    natives gives 0.61;
  - over all of gen 7 it is 0.75 (bin-mean) vs 0.66 (OLS).
- **The pilot.** The census's bin-mean exponents are 0.71-0.91 (000011001001011 0.91; the plan has 0.90), matching the
  plan within 0.03. The OLS exponents on the same natives are 0.59-0.76 for the ten largest owners.
- The g7 entries of 111000010101001 (3.52 / 4.56) rest on 54 natives and are not meaningful.
- **There is no contradiction.** 0.74-0.90 is a law for the **mean** cost (what CPU projections need); 0.51-0.66 is a
  law for the **typical** native.
- **Size range is a minor effect.** Restricting to fewer than 1e5 points changes the large owners' exponents by at
  most 0.02, except 000011001001011 (bin-mean 0.86 to 0.68, OLS 0.55 to 0.32), whose top-decade natives are few and expensive.
- **Neither global law captures the hot owner's superlinear top.** Its mean seconds per native are:

  | mean points | mean seconds |
  |---:|---:|
  | 328 | 0.070 |
  | 3,458 | 1.10 |
  | 18,438 | 10.98 |

  The top decade holds 66.7% of the hot owner's seconds; the top-decade exponent is 1.37.
- **Which projection to quote.** `projected_relative_cost_binned[E]` interpolates half-decade **arithmetic** means,
  which is the plan's kind of law and includes the superlinear top; it is the figure to quote.
  `projected_relative_cost_ols[E]` uses the geometric-mean exponent. It is more pessimistic (0.163 vs 0.100 D-only)
  because a smaller exponent makes small residual pieces relatively more expensive.

## 11. Point-space saturation and the potential check

**Saturation** (gen 7; union of Apply natives by record generation, 2000 draws per window, a sampling estimate):

`gen7/saturation.json` (draws per window 2000):

| gen | natives | est. new points | new points per native | union after |
|---:|---:|---:|---:|---:|
| 3 | 1532917 | 5.540e+11 | 361398.4 | 5.540e+11 |
| 4 | 1438703 | 6.375e+09 | 4430.8 | 5.604e+11 |
| 5 | 1399544 | 1.304e+09 | 931.4 | 5.617e+11 |
| 6 | 1129226 | 1.144e+10 | 10126.7 | 5.731e+11 |
| 7 | 448938 | 3.255e+06 | 7.3 | 5.731e+11 |

hot owner 011101110111000: g3 n 56254 new frac pts 3.9% cpu 2.1%; g4 n 74509 new frac pts 2.1% cpu 0.7%; g5 n 45364 new frac pts 1.1% cpu 0.4%; g6 n 71170 new frac pts 2.3% cpu 1.1%; g7 n 26440 new frac pts 0.9% cpu 0.3%

Reading [M]:
- **The union of native point space has nearly stopped growing.**
  - 5.54e11 points through g3, then +6.4e9 (g4), +1.3e9 (g5) and +1.14e10 (g6);
  - **+3.3e6 at g7: 7.3 new points per native** over 448,938 natives.
- **The hot owner re-inspects covered space.** CPU-weighted, only 2.1% (g3), 0.7% (g4), 0.4% (g5), 1.1% (g6) and
  0.3% (g7) of its native CPU lands on points not in the union of earlier natives.
- This is the same redundancy that G2' removes. The points-weighted totals are dominated by a few giant, cheap
  domains; the CPU weighting is the relevant one.

**Global-potential check** (`census potential`; observational only). Edges are aggregated to (phase, owner) nodes,
and a positive cycle is "unresolved", not a proof of nontermination:

| checkpoint | nodes | creator\|P graph: node pairs, positive pairs, positive cycle, max potential | other graphs (creator\|A, creator\|R, all_transitions\|A,P,R) |
|---|---:|---|---|
| v2 gen 3 | 8040 | 71,348 / 328 / none / 19 | all have positive cycles |
| v2 gen 6 | 8040 | 72,434 / 343 / none / 19 | all have positive cycles |
| v2 gen 7 | 8040 | 72,456 / 343 / none / 19 | all have positive cycles |
| C-5F | 680 | 3,110 / 65 / none / 9 | all have positive cycles |
| four-loop FG, BMW, H, X | 124-328 | none; max potential 2, 3, 2, 2 | mixed: H and X all_transitions\|P also acyclic (max 2) |

On P = A + R the creator graph admits a potential with maximum 19 at every v2 generation [M]. This neither proves
nor refutes termination; it is the observational consistency check the plan asked for.

## 12. Q3: guard and coefficient factor census (items 2 and 3)

**The run.** `rustred owner-domain-scan --factor-census [--factor-census-numerators]`, read-only. Symbolica's native
`Factorize::factor` runs on every distinct rule polynomial of the installed owner programs. Each irreducible factor
over the integer ring is classified by its degree in the index variables n and by where the base variables (d,
kinematics) appear. No walk, proof or cache is produced. Input: the v2 selection, `--unbounded-rank`, `max_terms`
4096. Both runs are serial: rustred requires every inner pool to be 1.
- plain: 125 s wall, 5.5 GiB max RSS;
- with numerators: 303 s wall, 5.6 GiB max RSS.

**Why `coefficient_denominator` and `term_denominator` had identical rows (item 3).** They are the same polynomials by
construction:
- Preparation stores each RHS term's denominator as a copy of its coefficient's denominator
  (`crates/rustred-core/src/solver/candidate_reduction/preparation/shared.rs` lines 200-213):
  `denominator = admit_native_polynomial_result_with_limits(term.coefficient.denominator.clone(), ..)` sits next to
  `coefficient = admit_native_result_with_limits(term.coefficient, ..)`.
- Neither admission changes the raw polynomial (`algebra/indexed/context/values.rs` 235-267).

The first census pushed both, so one polynomial family was tabulated twice under two names. This was not double
counting within a role, because roles are tabulated separately and never summed, but the second row carried no
information.

Commit `e8ebd501` drops the role. Instead it counts, per owner, the RHS terms whose coefficient denominator differs
from the stored term denominator: **0 of 1,667,335 [M]** in both runs, which confirms the coincidence on the real
data.

Run with numerators (`receipt-v4/q3/v2-factor-census-numerators.json`; the plain run `...-plain.json` gives identical rows for the other roles), summarized by `q3_summary.py` with the gen-7 CPU shares of `receipt-v4/gen7/cost.json`:

owners 67, rules 17975, affine cases 242, rhs terms 1667335; limits {'coefficient_numerators': True, 'max_terms': 4096}; prepare 130.7 s, census 168.2 s

coefficient denominators differing from the stored term denominator: 0 of 1667335 rhs terms (the term denominator is stored as a copy of the coefficient denominator, so 'term_denominator' covers both)

| role | occurrences | distinct polys | skipped/failed | polys all-factors affine in n, integer coefficients (occ.) | polys all-factors at most affine in n (occ.) | CPU-weighted affine-only | CPU-weighted index-affine | distinct irreducible factors (sum over owners) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| coefficient_numerator | 1667335 | 496669 | 0/0 | 54.5% | 71.1% | 41.5% | 57.3% | 330596 |
| equality | 258 | 129 | 0/0 | 100.0% | 100.0% | 100.0% | 100.0% | 129 |
| excluded_conjunction | 32037 | 2122 | 0/0 | 98.6% | 98.6% | 91.4% | 91.4% | 2126 |
| term_denominator | 1667335 | 35567 | 0/0 | 38.6% | 81.6% | 24.8% | 66.3% | 5845 |

Irreducible factors by class (occurrence-weighted: each factor counted once per rule occurrence of its polynomial):

| role | constant | base_only | affine_index | affine_index_base_offset | affine_index_base_slope | nonlinear_index |
|---|---:|---:|---:|---:|---:|---:|
| coefficient_numerator | 45.7% | 3.2% | 21.2% | 11.3% | 0.7% | 18.0% |
| equality | 0.0% | 0.0% | 100.0% | 0.0% | 0.0% | 0.0% |
| excluded_conjunction | 0.0% | 0.0% | 98.6% | 0.0% | 0.0% | 1.4% |
| term_denominator | 24.0% | 3.0% | 34.1% | 31.4% | 0.4% | 7.1% |

Reading [M unless marked]:
- **Guards are affine in n.**
  - Every equality guard factors into affine-in-n factors without base variables: 258 occurrences, 129 distinct.
  - 98.6% of excluded-conjunction occurrences do the same (91.4% weighted by gen-7 Apply CPU share). The remaining
    1.4% carry a quadratic-in-n factor.
  - So the plan's condition for the conditional "factor atlas for guards" (W4.4: "if Q3 shows mostly linear factors")
    is met for guards. Whether an atlas pays off is not measured here [E].
- **Term (= coefficient) denominators are mixed.** 35,567 distinct polynomials.
  - 38.6% of occurrences factor into constants, base-only factors and base-free affine-in-n factors only.
  - 81.6% are at most affine in n once base-dependent offsets such as (d - n_10 - 4) are allowed.
  - Factor occurrences split into constant 24.0%, base-only 3.0%, affine 34.1%, affine with base offset 31.4%,
    affine with base slope 0.4%, and nonlinear in n 7.1%.
  - Weighted by owner CPU the affine shares drop (24.8% and 66.3%): the CPU-heavy owners have the less linear
    denominators.
- **Numerators are far larger and less linear.** 496,669 distinct polynomials; 18.0% of numerator factor occurrences
  are nonlinear in n. The walk reads only the zero/nonzero decision of coalesced numerators, which is the N1 modular
  zero-certificate lever, not an atlas [E].
- No polynomial was skipped (above 4096 terms) or failed to factor in either run.
- **Reproducibility.** The results equal the first, dirty-tree run for every figure except one: "distinct irreducible
  factors" of the numerator role is 330,596 now vs 330,597 then. This is a one-factor difference in a hash-based
  distinct count, not investigated.

## 13. Checkpoint facts

- gen3: domains 28799586, records 16882169, live 14713699, buckets 8040, phase {"Apply": [5696293, 560, 0, 9671856626678.0], "Route": [23103293, 93773, 0, 99360447905251.0]}, lag q [378.0, 2671.0, 12645.0, 32001.0, 61394.0], out of order 53998
- gen6: domains 68873730, records 43166818, live 35396232, buckets 8040, phase {"Apply": [17739377, 627, 0, 11379084383363.0], "Route": [51134353, 93874, 0, 108881118127078.0]}, lag q [337.0, 2216.0, 9714.0, 26715.0, 87084.0], out of order 124159
- gen7: domains 74156033, records 45889639, live 37907667, buckets 8040, phase {"Apply": [19366066, 627, 0, 11379870416260.0], "Route": [54789967, 93874, 0, 108882480863254.0]}, lag q [330.0, 2089.0, 9455.0, 26500.0, 87084.0], out of order 131316

At gen 7, Apply native seconds are 241,437 against 1,951 for Route (`gen7/stats.json`).

## 14. Corrections to earlier summaries

- **Handoff section 7.8, pilot hot owner.** The C-HOT pilot's `hot_owner` rows refer to 011101110111000, which carries
  1.3% of the pilot's Apply CPU. The pilot's hot owner by CPU is 000011001001011 (section 6). Pilot all-owner numbers
  are unaffected.
- **Handoff section 7.8, conservative fix.** The handoff said the fix "changes gate shares by <= 0.12 pp". It changes
  gate and fully-covered shares by exactly 0; only the mean uncovered fraction and projected costs move, by up to the
  unevaluated weight.
- **Handoff section 7.8, per-owner rows.** The rows for 000011001001011 at gen 7 ("1.6% of draws: 18.9% D-only,
  36.8% hull") came from a `summarize.py owners` that skipped unevaluated draws. Counting its 7 infinite draws as
  failures, which is the rule of `census cover`, gives 1.7% of draws, 17.6% D-only and 34.3% hull. The hot owner's
  share of draws is 64.4% (3,863 of 6,000), not 64.5%.
- **Handoff section 7.8, four-loop.** The handoff expected the four-loop cover numbers to "change materially". Gates
  and fully-covered shares did not change. The mean uncovered fractions and projected costs did (section 1).
- **Handoff section 7.8, Q3.** The "Denominators: 35,567 distinct" figure is correct, but it describes one polynomial
  family, not two: coefficient and term denominators are identical.
- **Plan section 1, cost law.** "Within an owner, cost ∝ points^0.74-0.90" is correct as a law for the mean cost of
  the pilot's owners (bin-mean estimator). It is not the per-native OLS exponent, which is 0.51-0.66 for v2's large
  owners and 0.59-0.76 on the pilot's ten largest owners. The v2 figure 0.74 is the gen-6 segment only.
- **Census receipt `receipt/`.** It is superseded by `receipt-v4/`. Its binary's source was never verified: it was built
  9 s before commit 1df38d59.

## 15. Implications for W1-W5 and the D-session

- **D2 / W4.2 [M, with E projections].** G2' against merged Native anchors covers 61.6% of Apply CPU fully and puts
  70.1-89.0% under the gate. Projected Apply seconds are 0.06-0.10x on the same domain set [E]. General union cover
  (D2 proper) adds about 15 pp of fully covered CPU but needs pending covers (cycles). G2' is the sound first step and
  carries most of the benefit.
- **D3 (C2 vocabulary).** One-piece hull residuals in today's vocabulary match or beat A/R rectangles:

  | vocabulary | gate share | projected cost [E] |
  |---|---:|---:|
  | hull (today) | 89.0% | 0.057 |
  | A/R (C2) | 84.8% | 0.057 |
  | hull + C2 (`hull_c2`) | 90.6% | 0.048 |

  C2 is therefore optional for G2' [E]. It is worth about 15% of the residual cost on top of the hull and costs a
  representation change everywhere (plan section 3.10).
- **W4 gates.** The four-loop FG, H and X controls show no G2' opportunity (mostly infinite or uncovered queries).
  Judge W4.2 on C-5F (about 44% fully covered) and C-HOT-sub, and keep the four-loop runs for soundness.
- **W0.11.** Drop the hot-owner closure import: about 0 benefit [M]. Treat C-HOT as a 000011001001011 pilot when
  interpreting per-owner results.
- **P-anchors (W4.4).** No trigger: escape shells are 8.2e-5 of pending points [M].
- **Factor atlas (W4.4).** The guard condition is met [M]; payoff not measured [E].
- **Cost projections.** Quote the binned (arithmetic-mean) law. The plan's 0.74-0.90 stands as a mean-cost law. The
  hot owner is superlinear in its top decade, so residual cuts on its largest domains save more than either global
  exponent suggests [E].

## 16. Reproduce

```
wt=/common/dev/rustred/.claude/worktrees/fable51-compact
# census binary (tools/research/census/README.md), from a committed tree
cd $wt/tools/research/census && flock -w 14400 /common/dev/rustred/TMP/locks/build-1.lock nice -n 5 taskset -c 16-31,272-287 \
  env TMPDIR=$wt/TMP CARGO_TARGET_DIR=$wt/TMP/census-target nix develop ../../.. --command cargo build --release --locked --offline
# rustred binary for Q3
cd $wt && flock -w 14400 /common/dev/rustred/TMP/locks/build-1.lock nice -n 5 taskset -c 16-31,272-287 \
  env TMPDIR=$wt/TMP nix develop --command cargo build --release --locked --offline -p rustred-app --bin rustred
# batch (about 48 min on 32 CPUs), then Q3 (about 7 min, serial)
nice -n 19 taskset -c 16-31,272-287 bash $wt/tools/research/census/w0_batch.sh CENSUS_BIN OUT
W0_ONLY=q3 nice -n 19 taskset -c 16-31,272-287 bash $wt/tools/research/census/w0_batch.sh - OUT RUSTRED_BIN
# tables of this note
python tools/research/census/w0_tables.py OUT; python tools/research/census/summarize.py cost OUT/gen7/cost.json
python tools/research/census/q3_summary.py OUT/q3/v2-factor-census-numerators.json OUT/gen7/cost.json
```

Tests: `census` crate 5/0 (`cargo test --release --locked --offline` in `tools/research/census`);
`rustred --lib factor_census` 1/0; `rustred-app --lib owner_domain` 33/0 (Rust sources of 7a36c606, unchanged since; log `.claude/worktrees/fable51-compact/TMP/test-q3v4.log`); `cargo fmt --all -- --check` clean.
