# Bebe terms-first: measured content candidate

This is a modest rule-content/Pareto candidate for coordinator gating, **not a universal winner or a proposed default shipment**. Only the IBP source-row visit order changes: fewest source terms first, original Reference seeds, stock SpiredUncut integral order and FirstValid. No runtime sorting, optional portfolio or primary-score pre-screen is used.

The actual owner is `L4T08-d261f9c2d25e2dac` (bebe, P4L_J1, support `10111111001000`): **239 rules, LP1**, with the original residual key `[1,0,1,1,1,1,1,1,0,0,1,0,0,0]`. The measured libraries retain the original17 terminal values. The baseline is the freshly generated234-rule first-hit bebe owner in Diamond plus gated bubu, not the earlier globally rank-gated bebe variant.

## Contents and ordering

- `owners/L4T08-d261f9c2d25e2dac.rrbin`: actual generated native owner payload.
- The adjacent owner JSON: actual generator report, LP/residuals, backend, discovery recipe and native report TOML.
- `discovery.json`: pure terms-first source recipe; no portfolio policy.
- `metadata/rule-order.json`: explicit original RuleID order, chart by chart. The payload retains this FirstValid order; equal cases are not deduplicated.
- `metadata/terms-first.native.json` and `metadata/terms-first.per-rule.json`: native structural metadata and derived A/B/C/raising metrics, with original IDs preserved.
- `baseline/`: counterpart fresh-control owner report, native metadata and explicit order; `REFERENCE.json` points to its original payload/library. The baseline library is not copied here.
- `results/`: measured comparison, exact original commands/source commits and copies of the three already-passing acceptance/T-BFS reports. No passed science was re-executed for packaging.

## Regeneration

The measured generation source is **`5bdd53efddbf862b045126182a53e066f1fb0487`**. Reduction measurements used engine **`6d53b5a3`**. The CLI backend pass-through change **`4d375e1c`**, or equivalent support for `--exact-backend sparse-target-factorized`, is needed to reproduce the selected exact materialisation backend. The full bounded selector, trial-log or pre-screen changes are **not required to apply this saved payload**, and are not part of this candidate's reduction behaviour.

Use the existing licensed environment without printing it. ROOT supplies the assigned CPU and monitored unlimited-memory runtime. Set portable paths before the command; `RR_SRC` is a compatible source checkout, `LP_FROM` is the original owner-report directory supplying LP1, and `DATA_DIR` is this handoff directory. Outputs must stay in Astra.

```sh
: "${RR_BIN:?compatible rustred executable}"
: "${RR_SRC:?compatible RustRed checkout}"
: "${OUT:?new output directory}"
: "${LP_FROM:?original owners-1-4 reports}"
: "${DATA_DIR:?this bebe-terms-first directory}"
: "${CPU:?ROOT-assigned CPU}"
: "${ASTRA_HOME:?Astra workspace}"
case "$OUT" in "$ASTRA_HOME"/*) ;; *) echo 'OUT must be inside ASTRA_HOME' >&2; exit 2 ;; esac

taskset -c "$CPU" "$RR_BIN" p-generate \
  --families "$RR_SRC/crates/rustred-core/data/p/p-families.v1.json" \
  --max-loops 4 --owner L4T08-d261f9c2d25e2dac --threads 1 \
  --out "$OUT" --lp-from "$LP_FROM" --max-seconds 600 \
  --exact-backend sparse-target-factorized \
  --discovery "$DATA_DIR/discovery.json"
```

The original exact argv is preserved in `results/GENERATION-RUN.json`. Neither `--integral-order` nor `--force` is passed; no terminal-basis replacement is requested. The pure first-hit control is the same command with the `--discovery` pair omitted. The optional diagnostic `--trial-log` used in the measured run is not needed to generate this content.

## Measured before/after

One thread, cold reduction, census enabled; times are indicative on a shared host. The rank8 control is the earlier current-library run. The other controls use freshly generated/matched first-hit content. Frontier is the recorded maximum frontier, not a new per-layer-width claim.

| Cell | Owner content | Reduction s | Kernel peak RSS GB | Keys | Edges | Frontier |
|---|---|---:|---:|---:|---:|---:|
| NO3 d2_r6 | First-hit |68.50|1.255|620140|6934883|247715|
| NO3 d2_r6 | Terms-first |59.18|1.237|607136|6548766|240962|
| NO3 d2_r8 | First-hit |419.04|3.898|2074707|53680196|1019358|
| NO3 d2_r8 | Terms-first |424.84|3.729|2023371|50355493|990384|
| NO2 d0_r6 | First-hit |35.90|0.936|317606|4426927|136596|
| NO2 d0_r6 | Terms-first |36.20|0.935|318904|4403515|137482|

NO3 has smaller graphs and lower RSS; rank8 has **no wall-time gain**. NO2 is neutral, with slightly more keys and slightly fewer edges. Broader reference-grid validation/integration remains the coordinator's gate.

Generation took8.016s versus8.860s for the fresh first-hit control. Static content changes explain why this merits further measurement without establishing a universal win:

| Property | Fresh first-hit | Terms-first |
|---|---:|---:|
| Rules |234|239|
| Total RHS terms |6551|6536|
| RHS terms, median / maximum |17 /232|17 /181|
| RHS terms raising a support line |3682|3476|
| Support-line raising occurrences |4223|3831|
| Numerator-raising RHS terms |550|635|
| First/root rule A/B/C |19 /2 /0|8 /1 /0|
| Exception branches |416|427|

A is RHS count; B is distinct raised support lines; C is the positive maximum net inactive-degree shift. Symbolic coordinates use offsets and numeric coordinates use degree differences as in the native selection-quality arithmetic. These are scores, not domain-wide rank bounds. Exact case-text occurrence matching is retained in `results/COMPARISON.json`; it does not assert algebraic case equivalence or merge duplicate cases.

## Existing acceptance and exact references

All three measured candidates have PReduction **value lines byte-identical** to their declared reference after excluding only path-dependent `condition` lines. Their T-BFS certificates hold with strict descent, no repeated keys, phase regressions or foreign same-phase children. Existing reports are copied, not rerun:

| Cell | Local acceptance | Local T-BFS certificate | Exact value reference |
|---|---|---|---|
| NO3 d2_r6 | `results/acceptance/NO3d2r6/ACCEPTANCE.json` | `results/acceptance/NO3d2r6/certificate.json` | `/shared/vhirschi/t3code/vakint_dev/p_integral_dev/SCRATCHPAD/m5-r0/exec/wp0/goldens/preduction/P4L_NO3_L4_d2_r6/preduction.txt` |
| NO3 d2_r8 | `results/acceptance/NO3d2r8/ACCEPTANCE.json` | `results/acceptance/NO3d2r8/certificate.json` | `/shared/vhirschi/t3code/astra_5L/m5-rule-search/experiments/rank8/NO3d2r8-shipped-reference/preduction.txt` |
| NO2 d0_r6 | `results/acceptance/NO2d0r6-terms-first/ACCEPTANCE.json` | `results/acceptance/NO2d0r6-terms-first/certificate.json` | `/shared/vhirschi/t3code/vakint_dev/p_integral_dev/SCRATCHPAD/m5-r0/exec/wp0/goldens/preduction/P4L_NO2_L4_d0_r6/preduction.txt` |

`results/RESULTS.json` gives exact baseline/candidate run directories and metrics, and `results/GENERATION-RUN.json` contains the executed generator argv. Full raw reductions remain under `/shared/vhirschi/t3code/astra_5L/m5-rule-search/experiments/generation/terms-first-20261110T2256/`. Goldens are reused unchanged. Rank8's reference is a completed shipped-B1 result, not a newly designated WP0 golden.

## New route-map context: b_static

A subsequent matched experiment applies this same saved239-rule payload on the coordinator's experimental b_static package (maps commits e4312e31/24fda40a/809f5a54). The baseline has Diamond-only owner content, with no gated-bubu substitution. All new maps, the3Insertion presentations and the actual same17-terminal vector are retained; only bebe changes.

| Cell | Baseline→candidate time s | Kernel peak RSS GB | Keys change | Edges change |
|---|---:|---:|---:|---:|
| NO3 d2_r6 |72.08→53.52|1.223→1.233|+1.15%|+2.40%|
| NO3 d2_r8 |283.40→290.82|3.435→3.464|+0.56%|+1.33%|
| NO2 d0_r6 |18.97→16.64|0.727→0.706|−9.71%|−16.47%|

All6runs pass exact value/T-BFS/descent, with3reconstruction primes and1verification prime each. Commands, actual raw paths and exact numbers are in results/maps-b-static/RESULTS.json. The faster NO3rank6 time is indicative on the shared host and is not a structural graph win; its cause is not assigned. On this context NO2 improves and NO3 graphs/RSS slightly regress. This reverses the old context's graph tradeoff and reinforces that this is a measured candidate, not a universal default. Compare only within each matched library pair; cross-library deltas are not attributable to bebe alone.

## Artifact-format integration is pending

The required format is the coordinator's read-only `notes/artifact-ordering.SPEC.md`. This handoff provides explicit order, FirstValid policy, per-rule native structural data and measured provenance as **experiment sidecars**. It makes no source API/schema-integration claim and does not change production content/cache identity.

Coefficient polynomial degree and epsilon-pole flags remain explicitly **null**. Exact backend is recorded at owner level but not per saved rule. Generic rank shifts may be unknown at mixed fixed/symbolic coordinates. No missing field is fabricated. Derived A/B/C scores are supplied as sidecar data, not newly embedded in the payload. Production embedding/versioning, step-cache identity, persisted verified routes/symmetry groups, per-layer per-rule hit counts, ISP-basis symmetry-closure fractions and neighbour transport-map metadata remain future integration work. See `FORMAT-STATUS.json` for the requirement-by-requirement status.

This directory is data for a local branch/commit and coordinator review. It does not replace the installed library or select a new default.
