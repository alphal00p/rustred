# Bebe content choices on fixed b_static maps

All four variants pass exact values, T-BFS and descent on all three cells. Only condition lines are excluded from canonical value comparisons. Controls are reused; reduction engine6d53b5a3, cold1thread, caches off. Times are indicative on the shared host.

| Cell | Variant | Reduction s | Kernel RSS GB | Keys | Edges | Frontier |
|---|---|---:|---:|---:|---:|---:|
| NO3d2r6 | b_static | 72.08 | 1.223 | 599260 | 5388367 | 233831 |
| NO3d2r6 | terms-first | 53.52 | 1.233 | 606127 | 5517846 | 238022 |
| NO3d2r6 | AC-trace256 | 70.49 | 1.218 | 608508 | 5578536 | 236169 |
| NO3d2r6 | CA-trace256 | 46.13 | 1.211 | 599472 | 5472564 | 233832 |
| NO3d2r8 | b_static | 283.40 | 3.435 | 1931579 | 34848442 | 914931 |
| NO3d2r8 | terms-first | 290.82 | 3.464 | 1942391 | 35313419 | 924086 |
| NO3d2r8 | AC-trace256 | 287.06 | 3.460 | 1941758 | 35542090 | 914660 |
| NO3d2r8 | CA-trace256 | 283.36 | 3.424 | 1916576 | 35012436 | 909263 |
| NO2d0r6 | b_static | 18.97 | 0.727 | 194530 | 1652835 | 84491 |
| NO2d0r6 | terms-first | 16.64 | 0.706 | 175648 | 1380669 | 75009 |
| NO2d0r6 | AC-trace256 | 16.62 | 0.696 | 178058 | 1444421 | 76215 |
| NO2d0r6 | CA-trace256 | 15.86 | 0.691 | 170063 | 1345507 | 72710 |

## Generation cost, kept separate from reduction

| Owner content | Rules | Native generation s | Payload bytes | Source |
|---|---:|---:|---:|---|
| b_static | 234 | 8.860 | 3640693 | 5bdd53ef |
| terms-first | 239 | 8.016 | 3782302 | 5bdd53ef |
| AC-trace256 | 265 | 160.233 | 1934685 | cafebad9 |
| CA-trace256 | 264 | 163.481 | 1858338 | cafebad9 |

All candidates preserve values on these three cells. Owner-content gains depend on parent inflow and final maps. Compare keys/edges and memory as well as timing; no universal library default or promotion is selected here.

The265-rule AC and264-rule CA payloads are different. Both keep the original LP1 residual, maps/Insertion bases and17 terminals. FirstValid follows each saved rule order; there is no runtime portfolio, sorting or per-cell policy choice. Full exact metrics, guard peaks, actual primes, references and raw command/output paths are in FOUR-WAY.json and the per-experiment RESULTS files.
