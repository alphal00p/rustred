# b_static + AC trace256-selected bebe

Status **COMPLETE_PASS**; new reductions completed 3/3. Accepted baseline reductions were reused, not rerun.

This candidate has265 rules, the original LP1 residual and the same17-terminal vector. It selects per-case alternatives by A=RHS terms then C=positive maximum net inactive-degree shift. It is not the old239-rule pure terms-first owner. The static maps, all Insertion bases and every other owner remain unchanged.

Generation: source cafebad9, SparseTargetOnlyFactorized, stock SpiredUncut, Reference baseline plus8 true shell1/2 source/axis/sign alternatives, pre-screen on,30s cooperative case cap, optional exact trace at most256 rows/200000 terms. Native generation160.233s. Application is saved FirstValid order; no runtime portfolio or sorting.

| Cell | Variant | Reduction s | Kernel RSS GB | Guard sampled group GB | Keys | Edges | Frontier | Primes | Value/T-BFS |
|---|---|---:|---:|---:|---:|---:|---:|---:|---|
| NO3d2r6 | earlier b_static control | 72.08 | 1.223 | 1.168 | 599260 | 5388367 | 233831 | 3 | PASS |
| NO3d2r6 | AC trace256 | 70.49 | 1.218 | 1.162 | 608508 | 5578536 | 236169 | 3 | PASS |
| NO2d0r6 | earlier b_static control | 18.97 | 0.727 | 0.721 | 194530 | 1652835 | 84491 | 3 | PASS |
| NO2d0r6 | AC trace256 | 16.62 | 0.696 | 0.697 | 178058 | 1444421 | 76215 | 3 | PASS |
| NO3d2r8 | earlier b_static control | 283.40 | 3.435 | 3.430 | 1931579 | 34848442 | 914931 | 3 | PASS |
| NO3d2r8 | AC trace256 | 287.06 | 3.460 | 3.450 | 1941758 | 35542090 | 914660 | 3 | PASS |

Exact PReduction value bytes are compared to WP0 goldens for NO3d2r6 and NO2d0r6, and to the completed shipped-B1 RustRed reference for NO3d2r8. Only condition lines are excluded. T-BFS, strict descent and zero cache hits are required. References, commands, source/options, raw/guard paths and actual prime/degree/bit counts are in RESULTS.json and each new ACCEPTANCE.json.

Times are indicative because controls were measured earlier on the shared host. Kernel peak RSS and sampled guard group peak are different measurements. No baseline rerun, retry or package promotion is implied.

## Result

AC trace256 reduces NO2 keys/edges/RSS versus the earlier b_static control, but increases NO3 keys/edges at both ranks; NO3 RSS is slightly lower at rank6 and slightly higher at rank8. Three cells pass exact value/T-BFS/descent; no universal winner or package promotion is established. Generation cost160.233s is separate from reduction time.

| Cell | Time change | Kernel RSS change | Keys change | Edges change | Frontier change |
|---|---:|---:|---:|---:|---:|
| NO3d2r6 | -2.21% | -0.39% | +1.54% | +3.53% | +1.00% |
| NO2d0r6 | -12.40% | -4.27% | -8.47% | -12.61% | -9.80% |
| NO3d2r8 | +1.29% | +0.71% | +0.53% | +1.99% | -0.03% |
