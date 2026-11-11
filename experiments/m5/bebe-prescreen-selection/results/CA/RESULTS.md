# b_static + CA trace256-selected bebe

Status **COMPLETE_PASS**; new reductions completed 3/3. Accepted baseline reductions were reused, not rerun.

This candidate has264 rules, the original LP1 residual and the same17-terminal vector. It selects per-case alternatives by C=positive maximum net inactive-degree shift then A=RHS terms. It is not the old239-rule pure terms-first owner. The static maps, all Insertion bases and every other owner remain unchanged.

Generation: source cafebad9, SparseTargetOnlyFactorized, stock SpiredUncut, Reference baseline plus8 true shell1/2 source/axis/sign alternatives, pre-screen on,30s cooperative case cap, optional exact trace at most256 rows/200000 terms. Native generation163.481s. Application is saved FirstValid order; no runtime portfolio or sorting.

| Cell | Variant | Reduction s | Kernel RSS GB | Guard sampled group GB | Keys | Edges | Frontier | Primes | Value/T-BFS |
|---|---|---:|---:|---:|---:|---:|---:|---:|---|
| NO3d2r6 | earlier b_static control | 72.08 | 1.223 | 1.168 | 599260 | 5388367 | 233831 | 3 | PASS |
| NO3d2r6 | CA trace256 | 46.13 | 1.211 | 1.213 | 599472 | 5472564 | 233832 | 3 | PASS |
| NO2d0r6 | earlier b_static control | 18.97 | 0.727 | 0.721 | 194530 | 1652835 | 84491 | 3 | PASS |
| NO2d0r6 | CA trace256 | 15.86 | 0.691 | 0.667 | 170063 | 1345507 | 72710 | 3 | PASS |
| NO3d2r8 | earlier b_static control | 283.40 | 3.435 | 3.430 | 1931579 | 34848442 | 914931 | 3 | PASS |
| NO3d2r8 | CA trace256 | 283.36 | 3.424 | 3.425 | 1916576 | 35012436 | 909263 | 3 | PASS |

Exact PReduction value bytes are compared to WP0 goldens for NO3d2r6 and NO2d0r6, and to the completed shipped-B1 RustRed reference for NO3d2r8. Only condition lines are excluded. T-BFS, strict descent and zero cache hits are required. References, commands, source/options, raw/guard paths and actual prime/degree/bit counts are in RESULTS.json and each new ACCEPTANCE.json.

Times are indicative because controls were measured earlier on the shared host. Kernel peak RSS and sampled guard group peak are different measurements. No baseline rerun, retry or package promotion is implied.

## Result

CA preserves exact values on all3 cells and improves NO2 keys/edges/frontier/RSS versus reused b_static. NO3 rank6 has nearly equal keys but1.56% more edges; NO3 rank8 has fewer keys but0.47% more edges and slightly less RSS. Times are indicative; no universal default or promotion. CA is a distinct264-rule per-case C/A selection candidate, not the pure terms-first239-rule source order.

| Cell | Time change | Kernel RSS change | Keys change | Edges change | Frontier change |
|---|---:|---:|---:|---:|---:|
| NO3d2r6 | -36.00% | -0.96% | +0.04% | +1.56% | +0.00% |
| NO2d0r6 | -16.40% | -4.93% | -12.58% | -18.59% | -13.94% |
| NO3d2r8 | -0.01% | -0.34% | -0.78% | +0.47% | -0.62% |

Four-way comparison: FOUR-WAY.md / FOUR-WAY.json. Generation cost is reported separately.
