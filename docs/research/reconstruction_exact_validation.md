# Semi-numerical generation: exact validation and measured phase pilot

2026-09-19. This is a source audit and a completed **single-sector** diagnostic,
not a full-family benchmark, closure certificate, or implemented new backend.

## What runs today

[`search.rs`](../../crates/rustred-core/src/solver/search.rs) first tests exact
source leading terms. A direct hit bypasses modular discovery/materialization.
Otherwise a modular dependency trace selects exact source rows. The
[`semi-numerical materializer`](../../crates/rustred-core/src/solver/discovery/semi_numerical.rs)
probes their pivot chronology/support, reconstructs each target-row coefficient
with Symbolica, and compares the **complete row** against characteristic-zero
sparse elimination of that selected frame. Missing sampled support is not
silently accepted.

This internal exact validation belongs to **generation** and is included in
solver-only timings. It is different from optional artifact certification,
which also concerns original-source replay, applicability/guards, strict
descent, and declared-domain coverage. Neither process requires minimal masters.

Normal successful semi-numerical materialization performs one exact replay.
The first reconstruction error instead triggers lazy exact support recovery:
an exactly absent coefficient is skipped, while a required nonzero coefficient
retains the failure. That exact row is reused within the same attempt. A full
row mismatch can retry once with exact support; the new attempt does not retain
the old exact row, so it can replay again. Direct hits and the separate shared
fixed-integer-case solver must not be counted as these semi-numerical attempts.

The new `SemiNumericalExactReplayStarted/Finished` observer pair makes this
phase measurable without forwarding every sparse-row event. Failed replay is
explicit; finished replay is not yet accepted reconstruction or closure.
Existing full-parent logs do not contain this split.

## Completed H-sector pilot

Input: unchanged external H family from the matched four-loop matrix; sector
`0000110110`, natural ordering, 16 ordinary sources, authenticated zero census,
unbounded symbolic depth and finite-corner depth two. Native reconstruction
limits: degree 128, probes 200,000, attempts 4, primes 8. One optimized binary,
one worker on CPU 82, nested pools 1; two fresh pairs in sparse→semi then
semi→sparse order. Every run had a 180-second deadline; all completed with exit 0.

| Measurement | Pair0 sparse | Pair0 semi | Pair1 sparse | Pair1 semi |
|---|---:|---:|---:|---:|
| Solver core, seconds | 0.834502 | 1.215987 | 0.867125 | 1.322641 |
| Materializer intervals, seconds | 0.229134 | 0.609375 | 0.245689 | 0.671847 |
| Internal exact replay, seconds | — | 0.211860 | — | 0.230606 |
| Replay / semi materializer | — | 34.77% | — | 34.32% |
| Replay / semi solver core | — | 17.42% | — | 17.44% |
| Whole process wall, seconds | 2.21 | 2.61 | 2.20 | 2.76 |
| Whole process user / system CPU, seconds | 0.94 / 1.24 | 1.33 / 1.25 | 0.98 / 1.19 | 1.44 / 1.29 |
| Whole process peak RSS, KiB | 12,288 | 15,384 | 15,432 | 15,392 |

Core time includes sector preconditioning and the entire case solve, but
excludes family/source/zero preparation, snapshot encoding and comparison.
Materializer intervals end before canonicalization. Non-replay materializer
time includes support probes, reconstruction and wrappers—not pure
interpolation. Monotonic observer events are buffered structurally in memory;
no exact expressions are copied or logged inside the timed solve.

Each run produced 75 rules and 1 finite residual from 74 symbolic cases:
38 direct hits and 36 materializations, plus the shared numerical tail. Each
semi run had 36 attempts and 36 exact replays, with zero support recoveries,
failed replays or retries. The selected frames totaled 1,017 source-row
occurrences; largest frame 101 rows/246 columns, at most 6 active variables.

Both cross-backend pairs and both same-backend repeats matched all 2,017 RHS
coefficients exactly, with explicit numerator/denominator context checks,
plus case/guard/provenance/RHS-integral structure and finite residuals. No
oracle rules, terminal catalog, prior trace or saved solution entered solving.
The snapshots are diagnostic values, not published closing artifacts.

**Conclusion:** replay is measurable but not dominant here. Even subtracting
its entire cost with a hypothetical zero-cost replacement leaves semi core
times 1.004127/1.092034 s, above the paired sparse times. This does not predict
the split for full H, other parents, or the censored five-loop cube. Two shared-
host observations are not a confidence interval or general speedup claim.

Evidence: `TMP/reconstruction-phase-pilot.Cuwn0a/` contains the protocol,
generic input-driven client, raw events, statuses/resources, exact snapshots,
four comparisons, summary verifier and hashes. Binary SHA256:
`c4988b1357e70e57fbadd257d6e9456c3376caad7153238c944d30556e29cd91`.

## Exact source-span witness proposal — not implemented

Symbolica's native rational reconstruction uses a fresh-prime probabilistic
identity check. Deleting exact replay would therefore weaken the current
contract. A possible alternative is to retain native finite-field GPLU's
`L*U=A`, reconstruct source multipliers `lambda` with that **existing native**
reconstruction service, then verify the full exact product `lambda^T*A=r`.
Use native sparse solving/multiplication and explicit maps; require unit target
and zero forbidden columns. No custom reconstruction or linear algebra kernel.

Independent/dependent/empty source chronology must be handled explicitly;
native L rows and pivot ordinals are not interchangeable with integral columns.
The existing [`target-only backend`](../../crates/rustred-core/src/solver/discovery/target_only.rs)
already illustrates a native transposed triangular solve and full-row product,
but still performs partial characteristic-zero elimination. It is a useful
control, not evidence that a reconstructed-weight path will be faster.

An exact identity in prepared-source span is not automatically original-IBP
provenance or exceptional-face coverage. Source guards and canceled multiplier
denominator poles cannot be discarded. Preserve the present publication and
optional certification gates; sample agreement never supplies that authority.
Measure weight support/reconstruction/product costs before implementing a new
default, and expand phase sampling before attributing five-loop delays to replay.
