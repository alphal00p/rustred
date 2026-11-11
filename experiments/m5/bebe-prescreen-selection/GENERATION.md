# Bebe pre-screen result: two complete candidates, no generator still running

Both candidates retain exactly the original LP1 residual, stock SpiredUncut order, one worker and SparseTargetOnlyFactorized. New math source is cafebad9; only the explicit optional candidate selection differs. Native complete generation is not reduction/value/T-BFS acceptance. Those three-cell reductions are owned by perf_owner_map.

| Native generation | seconds | rules | total RHS terms | numerator-raising RHS terms | printed coefficient bytes |
|---|---:|---:|---:|---:|---:|
| FirstValid | 8.860 | 234 | 6551 | 550 | 1523267 |
| PureTermsFirst | 8.016 | 239 | 6536 | 635 | 1511874 |
| AC | 160.233 | 265 | 7166 | 482 | 570877 |
| CA | 163.481 | 264 | 7168 | 416 | 559654 |

| Complete pre-screen owner | cases | optional completions | exact admitted optional / novel | optional novelty | pre-screened, not admitted | trace-refused | completed lift seconds | guard seconds / sampled peak MB |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| AC | 264 | 2112 | 1209 / 131 | 10.84% | 639 | 100 | 4.777 | 165.37 / 48.99 |
| CA | 263 | 2104 | 1206 / 128 | 10.61% | 640 | 98 | 5.082 | 167.83 / 51.04 |

Optional novelty excludes mandatory baselines, pre-screened outputs, trace refusals and failed searches from the admission denominator. Exact dedup uses the native completed case/target/RHS/exceptions, not merely equal scores. Pre-screened records have only postcanonical pre-guard A/C features; no full admitted score is fabricated.

Common initial root: both improve from full baseline (A=19,C=0) to (A=8,C=0), inferred trial3 under the configured stable ordering. Later progress case ordinals are not aligned. Actual per-rule cases/targets/exceptions remain in native metadata, with every saved original ID retained in strict FirstValid order.

Bottleneck after trace limiting: discovery dominates (~152.33s AC /155.23s CA); completed exact lifting is only4.78s/5.08s. Native guard extraction is~0.10s each. Generation is~18x the FirstValid8.86s, so a runtime reduction win must pay for this generation overhead rather than being inferred from better local scores. Timings are shared-host indicative.

The previous unbounded optional trial6 caused an811-row target-block lift. Both old300s attempts remain recorded incomplete. The distinct trace256 variant rejects oversized optional traces before lifting while keeping the mandatory baseline unlimited. Original CA180s attempt returned an ordinary182s budget error; the explicitly authorized360s continuation completed in163.72s native wall time. Prior failure is unchanged; clock/host variability means the new larger budget did not force a longer actual run.

Rule properties relevant to5L: compared with FirstValid, both candidates have ~9% more total RHS terms but much smaller printed coefficient expressions (~63% fewer bytes) and fewer numerator-raising terms (AC482,CA416 vs550). CA and AC are different payloads. Rule count or RHS count alone does not establish runtime benefit; path volume, coefficient cost and value/T-BFS measurements decide.

Metadata: AC/CA.metadata.json from existing native exporter, AC/CA.per-rule.json including actual shifts, A/B/C, raised/lowered lines, exception and provenance counts, and AC/CA.explicit-order.json. Coefficient polynomial degree and epsilon-pole flags/order are explicitly null. Backend is owner-generation provenance, not guessed per-rule provenance. No payload was reordered or changed.

All generation/metadata scopes are terminal/inactive; CPUs296/297/302 released from these tasks. No further generation, cap extension, Reference-only fallback, library staging or reduction is performed by this owner. AC and CA payloads/reports were handed directly to perf_owner_map, which alone owns package/value/T-BFS reductions.
