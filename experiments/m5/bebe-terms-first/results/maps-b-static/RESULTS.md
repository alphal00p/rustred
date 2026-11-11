# b_static + terms-first bebe

Status: **COMPLETE_PASS**; completed 6/6 jobs.

Baseline is the exact static-map package with Diamond-only owner content. Candidate changes only bebe to the saved239-rule terms-first/Reference/FirstValid payload. All b_static presentations/maps and the actually equal17-terminal vector are retained. No gated-bubu owner is introduced.

| Cell | Variant | Reduction s | Kernel RSS GB | Guard sampled group peak GB | Keys | Edges | Frontier | Primes | Value/T-BFS |
|---|---|---:|---:|---:|---:|---:|---:|---:|---|
| NO3d2r6 | baseline | 72.08 | 1.223 | 1.168 | 599260 | 5388367 | 233831 | 3 | PASS |
| NO3d2r6 | terms-first | 53.52 | 1.233 | 1.228 | 606127 | 5517846 | 238022 | 3 | PASS |
| NO3d2r8 | baseline | 283.40 | 3.435 | 3.430 | 1931579 | 34848442 | 914931 | 3 | PASS |
| NO3d2r8 | terms-first | 290.82 | 3.464 | 3.466 | 1942391 | 35313419 | 924086 | 3 | PASS |
| NO2d0r6 | baseline | 18.97 | 0.727 | 0.721 | 194530 | 1652835 | 84491 | 3 | PASS |
| NO2d0r6 | terms-first | 16.64 | 0.706 | 0.682 | 175648 | 1380669 | 75009 | 3 | PASS |

Timings are indicative on a shared host. Each result is compared to its declared WP0 golden (r6) or completed shipped-B1 reference (r8), excluding only condition lines; T-BFS, strict descent and zero cache hits are required. Native loader failures never fall back to a different package.

Commands, raw output/guard paths, source commits, exact metrics, actual prime/degree counts and before/after percentages are in RESULTS.json. Existing guard samples and manual host/scope observations remain in their ordinary raw logs. Kernel peak RSS and sampled guard group peak have distinct meanings.

This is a measured combination experiment, with no package promotion. Do not attribute differences from older libraries to bebe: only the fresh b_static pair isolates this owner-content change.

## Result

On b_static, terms-first improves NO2 d0_r6 structurally (fewer keys/edges/frontier, lower RSS) but increases NO3 keys/edges/frontier and kernel RSS at both ranks. NO3 rank8 is also slower. NO3 rank6 faster observed wall time is indicative on the shared host and is not a graph-size win; its cause is not assigned. Do not promote a universal owner default from this three-cell tradeoff.

| Cell | Time change | Kernel RSS change | Keys change | Edges change | Frontier change |
|---|---:|---:|---:|---:|---:|
| NO3d2r6 | -25.75% | +0.87% | +1.15% | +2.40% | +1.79% |
| NO3d2r8 | +2.62% | +0.84% | +0.56% | +1.33% | +1.00% |
| NO2d0r6 | -12.28% | -2.82% | -9.71% | -16.47% | -11.22% |
