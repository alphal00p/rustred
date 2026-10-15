# Native rational-coefficient weighted lowering: isolated performance gate

## Result and scope

A narrow fast path using Symbolica's existing rational polynomial conversion
passes the automated scalar-lowering tests and six synthetic exact-comparison
benchmarks. The larger five-loop inputs take about one third of the existing
generic-coefficient lowering time. **This is not yet a whole-Vakint speedup**,
and the implementation remains isolated pending the actual witness-search gate.

The opportunity came from the delivered R2 Vakint cancellation controls:
repeated weighted polynomial lowering dominated a short profile of a difficult
mixed-denominator input. See
[the acceptance report](r2_d10_application_acceptance_2026-10-15.md). The new
path changes neither the integral order nor matching/admission decisions.

## Mechanism and exactness

The existing bounded denominator-grammar traversal identifies strictly
real-rational coefficients. After denominator translation, the candidate uses
Symbolica `try_to_polynomial(&Q, ...)`, requires an unchanged variable map, and
uses Symbolica `map_coeff` to return to the existing shared weighted-lowering
pipeline. There is no new CAS or reconstruction implementation.

Opaque functions, symbolic coefficients or masses, and complex/algebraic
coefficients retain the existing generic path. Original condition scanning,
pole guards, coalescence, input admission and resource checks are unchanged.
The fast path does not admit an input that was previously outside the saved
campaign's exact domain union.

All 28 automated scalar-numerator tests pass, including five new differential
and fault tests. The pre-existing ignored manual captured-input diagnostic is
not counted as automated coverage. Tests compare complete exact terms,
coefficients and conditions, including factored/expanded numerators, non-unit
mass, cancelled poles, fallback inputs and deliberately invalid variable maps.
The final test rerun took 1.51 seconds natively.

## Paired synthetic measurements

One optimized binary runs both converters with a scoped test-only override.
Each fixture has three alternating-order pairs. Every call checks exact output
and guards and verifies which conversion branch executed. All six exact gates
and all 36 timed calls passed. Setup is outside the per-call boundary.

Median milliseconds; ratios are generic time divided by rational time.

| Input | Expanded terms | Generic | Rational | Ratio |
|---|---:|---:|---:|---:|
| Four-loop identity routing, mass² 1 | 28 | 1.381 | 0.640 | 2.16× |
| Four-loop identity routing, mass² 3/2 | 28 | 1.521 | 0.693 | 2.19× |
| Five-loop identity routing, mass² 1 | 160 | 17.179 | 5.178 | 3.32× |
| Five-loop identity routing, mass² 3/2 | 160 | 18.875 | 5.566 | 3.39× |
| Five-loop unimodular shears, mass² 1 | 2,156 | 514.759 | 157.667 | 3.26× |
| Five-loop unimodular shears, mass² 3/2 | 2,156 | 535.922 | 167.245 | 3.20× |

The shear fixture uses signed elementary row additions, preserving an exact
unimodular loop-basis transformation. The test permits explicit bounded shear
specifications without rebuilding the engine. It lowers an expanded mixed
denominator product, not a complete IBP application or full witness search.
The final output cap is one; other scalar-lowering limits remain at their
defaults. Native benchmark time was 5.80 seconds, supervised time 6.221 seconds,
and driver total 6.505 seconds. Peak sampled process-tree RSS was 79.7 MB.
One native test thread ran on the leased CPU 28–31 set; reserved CPUs do not
mean four parallel polynomial conversions.

The larger generic times ranged 0.492–0.519 and 0.520–0.545 seconds; rational
times ranged 0.152–0.159 and 0.166–0.186 seconds. These are small same-process
mechanism measurements, not an end-to-end confidence interval or extrapolation
to arbitrary ranks.

Unchanged work-admission counters do not imply equal native computation cost:
Symbolica's Q conversion incrementally adds children where its generic
AtomField path can collect and sort in bulk. That difference is why actual
measurements, rather than the proxy counters alone, determine this comparison.

## Negative result retained

The original two-sweep dense fixture had 10,752 expanded terms. It failed the
existing common weighted-collection preflight at 16,047,625 requested work
units versus a 16,000,000 allowance, before either conversion could complete
the comparison. Four earlier cases passed, but that run is not a completed
benchmark. Its receipt remains under
`TMP/weighted-rational-lowering-paired-runtime-20261015/`.

Exactly one test-fixture correction replaced that dense transformation with
moderate elementary shears. No allowance was raised, and the old failing
transformation remains reproducible through the test input. This does not
claim that the optimization fixes the original preflight limitation.

## Reproducibility and next gate

Worktree: `TMP/weighted-rational-lowering-20261015`, based on `af64457c`.
Frozen optimized test executable:
`TMP/releases/20261015-weighted-rational-moderate/rustred-core-tests`.

SHA256: `3d029ea1ca1d6ce4a2b5a7a8ba6d6d2cd750f1e2f0ab414ec020e84388e7c149`.

Evidence: `TMP/weighted-rational-lowering-moderate-runtime-20261015/` contains
the exact command, matched measurements, complete gates and resource receipts.
The independent auditor rechecked the binary, 1,876 source/vendor hashes and
three dependency bindings. The successful optimized rebuild took 690.655
seconds; compilation is not part of any timings above.

This binary uses Symbolica `ef0db494533c87adb40356c996241680dc5a7bff` / 3.0.0.
The delivered Vakint integration uses Symbolica `400a53c4` / 3.0.1 and FeynKit
`024f96d6`. The next gate preserves that actual integration stack and compares
the existing `mixed_dotted_lines` native DOT input with fresh application
state in each arm. It must preserve candidate visitation and minimum-witness
selection, compare exact lowering/admission/guard evidence and final master
coefficients, and report package initialization separately. The override must
cover every witness-search call, including any threaded execution.

That whole-Vakint test is not yet implemented or passed. No production campaign,
artifact, numerical master evaluation or default policy was changed by this
isolated experiment.
