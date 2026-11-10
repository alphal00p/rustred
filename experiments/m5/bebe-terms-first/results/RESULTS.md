# Bebe pure terms-first content experiment

Native generator5bdd53ef; reduction engine6d53b5a3. One thread, cold, census enabled. All paired values match the declared golden/completed-shipped reference after excluding only path-dependent condition lines; T-BFS and strict descent pass. Original17terminals/LP1 retained.

| Cell | Rules | Reduction s | Kernel peak RSS GB | Keys | Edges | Frontier |
|---|---|---:|---:|---:|---:|---:|
| NO3 d2_r6 | First-hit control | 68.50 | 1.255 | 620140 | 6934883 | 247715 |
| NO3 d2_r6 | Terms-first | 59.18 | 1.237 | 607136 | 6548766 | 240962 |
| NO3 d2_r8 | First-hit control | 419.04 | 3.898 | 2074707 | 53680196 | 1019358 |
| NO3 d2_r8 | Terms-first | 424.84 | 3.729 | 2023371 | 50355493 | 990384 |
| NO2 d0_r6 | First-hit control | 35.90 | 0.936 | 317606 | 4426927 | 136596 |
| NO2 d0_r6 | Terms-first | 36.20 | 0.935 | 318904 | 4403515 | 137482 |

Timings are indicative on a shared host. The rank8 control is reused from the earlier current-library run; the other controls were freshly regenerated/matched. NO3 gains graph size/RSS; rank8 has no wall-time gain; NO2 is neutral, with slightly more keys and slightly fewer edges. No broad winner is claimed.

Generation completed8.016s versus8.860s firsthit.234→239rules; RHS total6551→6536,median17→17,max232→181. Support-raising terms3682→3476; numerator-raising terms550→635. RootABC19/2/0→8/1/0. Full per-rule metadata/explicit order: ../terms-first-comparison/COMPARISON.json.

5L lesson: prioritize cheap native source-row structure before paying for broad same-case portfolios. Static total term/rank counts do not predict every input's DAG; score emitted volume, keys/edges, coefficient replay and RAM together. The source-first lane is a candidate for integration, not a globally selected default.

The2/8/32screens produced useful diversity data but no complete artifact before their2100s caps. A separate phase-only reproduction located optional-trial exceptional geometry; no longer cap was used. A default-off primary-score pre-screen is under development.
