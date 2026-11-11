# Bebe bounded selector candidates

Two measured candidates for coordinator review, not a universal default. AC scores (RHS terms, positive maximum net inactive-degree change); CA reverses those priorities. Each keeps a mandatory input-order Reference baseline and tries8 source-order/real shell1/2 alternatives. The default-off pre-screen rejects only strictly worse known score prefixes after exact target matching and strict descent; ties/unknowns proceed. Optional traces are capped at256rows/200000terms BEFORE lifting; the mandatory baseline is unchanged.

Source cafebad9a5b47b28e53a6d8c8c5a2f1cc2553f06, reducer6d53b5a3. Owner L4T08-d261f9c2d25e2dac, P4L_J1/presentation167/support1277. AC has265rules and CA264. Both retain original LP1 residual [1,0,1,1,1,1,1,1,0,0,1,0,0,0] and the17-terminal library contract. Stored native FirstValid order is strict; no runtime portfolio/sort or case-key deduplication.

## Performance and exactness

All6new reductions pass exact PReduction value identity and T-BFS/strict descent (3reconstruction primes+1verification each); only path-dependent condition lines are excluded. References are WP0 goldens for rank6 and the actual completed shipped-B1 RustRed result for NO3rank8. The unchanged b_static controls and pureterms-first measurements are reused, not rerun. Cold,1thread,cachesoff; times indicative on a shared host; kernel peakRSS is distinct from sampled guard groupRSS.

| Cell | Variant | Time s | Kernel RSS GB | Keys | Edges |
|---|---|---:|---:|---:|---:|
| NO2 d0_r6 | b_static |18.97|0.727|194530|1652835|
| NO2 d0_r6 | AC |16.62|0.696|178058|1444421|
| NO2 d0_r6 | CA |15.86|0.691|170063|1345507|
| NO3 d2_r6 | b_static |72.08|1.223|599260|5388367|
| NO3 d2_r6 | AC |70.49|1.218|608508|5578536|
| NO3 d2_r6 | CA |46.13|1.211|599472|5472564|
| NO3 d2_r8 | b_static |283.40|3.435|1931579|34848442|
| NO3 d2_r8 | AC |287.06|3.460|1941758|35542090|
| NO3 d2_r8 | CA |283.36|3.424|1916576|35012436|

CA is strongest on NO2; the unchanged baseline retains fewer NO3 edges. Rank-first is not a universal winner. Compare content in its final map/basis context: these b_static results do not establish a cross-package speedup. The next assigned experiment applies shipped, pureterms-first and these candidates to the coordinator's gated combined package when delivered.

## Generation cost and transferable properties

First-hit234rules took8.86s; pure source-terms-first239 took8.02s. AC265/CA264 took160.23/163.48s with sampled guard peaks49.0/51.0MB. Optional-only canonical novelty is10.84%/10.61%; baselines, pre-screened and refused searches are excluded from admission denominators. The completed owners have ~9% MORE total RHS terms (7166/7168 vs6551), but fewer numerator-raising RHS terms (482/416 vs550) and ~63% fewer printed coefficient bytes. No benefit is inferred from rule count alone. Discovery dominates after the trace cut; completed lifting costs only~5s.

Historical +8 attempts without the trace cut hit300s in an811-row optional lift. Original CA180s trace-limited attempt returned a time-budget error; the recorded360s followup completed within164s actual native wall time. These outcomes remain in original run directories; no failure is hidden.

## Files, metadata and regeneration

variants/{AC,CA}/ contains actual owner payload, native report, exact measured invocation/options, complete result, discovery recipe, explicit originalRuleID order and per-rule structural metadata. Coefficient degree/epsilon-pole flags remain null; no guessed score or identity is supplied for a pre-screened trial. Exact backend is owner-generation provenance. Production artifact-v2 integration/versioning, metadata embedding and cache identities belong to the coordinator; these are historical B1 experiment sidecars, not a claim to the new format.

Use a compatible cafebad9 executable (including separate exact-backend pass-through4d375e1c), quietly sourced licensed environment and ROOT's monitored unlimited-memory admission. Set RR_BIN, RR_SRC, LP_FROM (original owner reports supplying LP1), DATA_DIR (this directory), OUT (new Astra output), CPU and MODE (AC or CA). Equivalent CLI regeneration; exact measured native API calls/options remain in each RUN.json:

~~~sh
case "$OUT" in "$ASTRA_HOME"/*) ;; *) exit 2 ;; esac
taskset -c "$CPU" "$RR_BIN" p-generate \
  --families "$RR_SRC/crates/rustred-core/data/p/p-families.v1.json" \
  --max-loops 4 --owner L4T08-d261f9c2d25e2dac --threads 1 \
  --lp-from "$LP_FROM" --out "$OUT" --max-seconds 360 \
  --exact-backend sparse-target-factorized \
  --discovery "$DATA_DIR/variants/$MODE/discovery.json"
~~~

Do not add an integral-order override. Do not overwrite accepted libraries or use old step caches after changing payloads/maps. Reference, command, prime, frontier and separate memory evidence is in results/{AC,CA}/RESULTS.json and results/FOUR-WAY.json. No knock-out closure or VERIFIED status is changed by this performance experiment.
