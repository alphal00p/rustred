# C-HOT-sub definition (W0.9 falsify lane, 2026-09-27)

Hot owner `011101110111000`, C-HOT inputs (manifest/owner base of
`TMP/qcd-feynman-d9d10-pilot-hot-owner/matrix-32fdec/hot-owner-physics-ordered/run/request.json`,
i.e. `campaigns/five-loop-dependency-closure/inputs`, read-only), one query
replacing the three C-HOT queries:

| Name | Query file | Box | Exact drain on the legacy engine [M] |
|---|---|---|---|
| **C-HOT-sub (primary)** | `TMP/w0/falsify/inputs/hotsub-r1a12.json` | full orthant, R <= 1, A <= 12, D free | 846 s whole / 739 s traversal; 1,020,597 natives, 1,385,250 domains; root closed; 0 frontiers |
| C-HOT-sub2 (secondary) | `TMP/w0/falsify/inputs/hotsub-r2a11.json` | full orthant, R <= 2, A <= 11, D free (superset of C-5F's box) | 993 s / 880 s; 1,167,377 natives, 1,551,099 domains; root closed; 0 frontiers |

Measured with binary `TMP/w0/falsify/bin/rustred-bce6772f` (sha256
bce6772f1b3a185dc33f12fbb851f87c64978ecddb683f2b786ad4bf43b2436b; fable_5_1
b15316b9 + an env-gated W0.9 change that is OFF unless RUSTRED_W0_G1_WIDEN is
set; flag-off counts equal 4a17f9c7 on FG/BMW/H/X and C-5F), W12 Ordered,
CPUs 264-275 / 276-287 (the two runs concurrently), nice 5, lookahead 256,
`--reuse-initial-d-bands --route-domain-overcover --unbounded-work`.
Runs: `TMP/w0/falsify/runs/exact-bce6772f-hotsub-{r1a12,r2a11}-w12/hot`.

Runner: `tools/research/w0_falsify/run_arm.py --family hot --queries <file>`
on branch `fable_5_1-v3-widen`.

Rejected (do not drain in 20 min on the legacy engine) [M]: r3a12 (W24: 626k
natives / 1.49M domains at 726 s traversal, still growing), r2a12 (W24: 1.38M /
2.15M at 1,136 s), r3a11 (W12: 937k / 1.68M at 1,010 s), r1a13 (W12: 1.21M /
1.88M at 1,010 s).
