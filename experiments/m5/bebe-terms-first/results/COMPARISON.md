# Bebe: input-order versus terms-first

Both generators completed rc0 with Reference seeds and FirstValid. Native rule order is preserved; this compares rule content only. Both retain LP1 and exactly the original residual key.

| Metric | Fresh first-hit input order | Pure terms-first |
|---|---:|---:|
| Rules | 234 | 239 |
| Generation s | 8.860 | 8.016 |
| RHS terms: total / median / max | 6551 / 17.0 / 232 | 6536 / 17 / 181 |
| Distinct raised support lines/rule: total / median / max | 721 / 2.0 / 8 | 696 / 2 / 8 |
| Positive max net numerator shift/rule: total / median / max | 10 / 0.0 / 1 | 11 / 0 / 1 |
| RHS terms raising a support line: total / median / max | 3682 / 6.0 / 224 | 3476 / 6 / 173 |
| Support-line raising occurrences: total / median / max | 4223 / 6.0 / 306 | 3831 / 6 / 218 |
| Numerator-raising RHS terms: total / median / max | 550 / 0.0 / 145 | 635 / 0 / 118 |
| Exception branches: total / median / max | 416 / 2.0 / 3 | 427 / 2 / 3 |
| Exception predicates: total / median / max | 418 / 2.0 / 3 | 427 / 2 / 3 |
| Source descriptors: total / median / max | 2481 / 4.0 / 136 | 2431 / 2 / 142 |

First/root rule ABC: input `[19, 2, 0]`; terms-first `[8, 1, 0]`.

Exact-case-text occurrence matches: 111; smaller/unchanged/larger RHS counts = 19/85/7. This does not equate cases algebraically or deduplicate them.

All medians/maxima, signed numerator shifts, per-term raised-line counts, provenance counts and raw exception text are retained in JSON. Coefficient degree and epsilon-pole order are unavailable from this structural exporter, explicitly null. The exact backend is known from owner records, not per-rule payload provenance.

These are static offset/size metrics, not measured reduction gains or domain-wide rank bounds. Timings are indicative on a shared host. ROOT owns the terms-first PReduction value/T-BFS and path-census experiment.

Commands and source commits are in COMPARISON.json. Native per-rule metadata and explicit original-ID orders are kept beside this report; no payload was modified.
