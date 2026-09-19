# Exact finite-terminal normalization

Terminal declarations, equivalent integral representatives and independent
masters are different objects. The first RustRed normalization service merges
only terminal equalities established by an exact change of loop momenta. It
does not import FMFT identities, certify a candidate program, prove completeness
or claim a minimal basis.

## Ownership and use

The generic service lives in
`rustred::reduction::terminal_normalization`. Given an `IntegralFamily`, its
finite declared `BTreeSet<IntegralKey>` and an `OrderingPolicy`, prepare a
`TerminalAliasPlan::independent_tadpole_products`. The plan exposes the original
declarations, canonical representatives, exact momentum witnesses and separate
eligibility/skip counts.

`CandidateReducer::install_terminal_aliases` is an explicit integration hook.
It checks the family fingerprint, ordering and exact raw terminal set once.
Installation requires an empty cache; call `clear_cache` explicitly if the
reducer has already been used. Even a failed reduction can leave valid partial
cache entries, so failure of a request is not evidence of an empty cache.
Preparing a plan alone changes neither the reducer nor its output.

The existing `terminals()` getter continues to return the original declarations,
preserving provenance and the offline catalog-key contract. The separate
`canonical_terminals()` getter describes possible output representatives.
Ordinary construction remains unchanged unless a caller installs a plan.

When an integral reaches a terminal, the reducer emits its one-hop
representative with coefficient one. The cache key and decomposition's target
remain the original integral. Ancestor coefficients then combine on canonical
keys through the existing Symbolica arithmetic and memoized rule applier. The
hot path neither rebuilds a plan nor replays its momentum witnesses.

## First supported mathematical class

The family must be a vacuum family without analytic power shifts. An eligible
terminal has exactly L positive indices and all remaining indices zero. Each
active denominator must be an integer linear momentum square minus one:

`D_i = (sum_j U_ij k_j)^2 - 1`.

Symbolica's native polynomial factorization proposes the linear momentum. Exact
squaring checks it; a truncated integer square root is never sufficient proof.
Native Symbolica matrix inversion and the existing RustRed replay service must
establish an integer momentum basis with determinant +1 or -1. These conditions
identify a product of L independent unit-mass tadpoles.

Products with the same sorted multiset of positive powers have the same
integral. The least existing declared key under the supplied ordering is the
representative. For each source, RustRed forms

`k_source = U_source^-1 U_representative k_representative`

and passes it to the existing generic momentum-map verifier. Every active
source denominator must map to its paired representative denominator with
unit scale and matching power, and the Jacobian must be unit. Inactive
zero-power auxiliary denominators need not map by a permutation. Conservatively,
any nonconstant nonzero condition produced by verification causes this first
service to retain the source rather than publish a conditional alias.

No alias introduces a new terminal or points to another alias. The positive
power multiset is unchanged, so common-mass restoration retains exactly the
same `(m²)^(sum(master powers) - sum(target powers))` factor.

Unsupported products stay in the raw output basis: external momenta, analytic
offsets, negative indices, non-unit masses, noninteger or nonsquare quadratic
forms, singular or non-unimodular bases and conditional maps are not silently
approximated. Malformed ownership or exact-algebra failures are errors.

## Validation and measurement contract

Small tests cover exact square replay, large integer shears, negative and
nonsquare content, non-unit Jacobians, rational rather than integer routings,
offsets, masses, numerator-bearing terminals, conditional geometry and stable
ordering. Independent tests include the two FG products described in the local
`EPSILON.md`, without using their FMFT projections as discovery inputs.

Application tests compare the normalized K3 result with an exact Symbolica
coalescing of the original result, including raised powers, negative indices,
zero integrals, mass factors, cache reuse and failed installation. These checks
supplement rather than replace numerical acceptance of saved four-loop programs.

On real saved programs, record raw and canonical counts, proposal/preparation
time, memory and deterministic witnesses. Consult the existing offline catalog
only *after* preparing aliases, as an independent exact comparison. Separately
measure first unseen application, cached repetitions and retained coefficient
terms; fewer terminals do not automatically imply faster rule generation.
No IBP regeneration is needed for these measurements.

### First release gate

The isolated service passed 15 tests (seven implementation and eight independent
adversarial tests) in 0.03 s. The candidate application module then passed all
16 tests in 0.06 s, including five new integration tests and all existing
guard/denominator/descent/budget checks. The combined `reduction::` filter passed
43 tests in 0.09 s with 6,160 KiB process peak RSS. These counts overlap and
these are small-test execution times, **not** four-loop solve or application
benchmarks. Compilation was outside the timed execution boundary.

Local evidence is retained in `TMP/terminal-alias-release-tests-final.log`,
`TMP/terminal-alias-integration-release.log` and
`TMP/terminal-alias-all-reduction.{log,time}`. An independent source and
mathematical audit approved the service and application hook. Saved-four-loop
application diagnostics are recorded below; activation in Vakint and its complete
numerical acceptance matrix remain pending. The hook is not enabled by default.

### Saved four-loop terminal census

An optimized, input-driven Rust client loaded the unchanged native candidate
programs, then prepared aliases independently of the offline master catalog.
Afterward, every proposed alias was checked against the stored exact catalog
projection using Symbolica. No IBP rules or FMFT values were generated.

| Input | Raw declared keys | Proven aliases | Canonical output keys | First preparation |
| --- | ---: | ---: | ---: | ---: |
| H | 386 | 74 | 312 | 0.340 s |
| FG | 145 | 39 | 106 | 0.155 s |
| BMW | 179 | 44 | 135 | 0.180 s |
| X | 445 | 80 | 365 | 0.372 s |
| Total | 1,155 | 237 | 918 | — |

All 237 aliases agree exactly with the independently supplied projections.
Four additional preparations per input reproduced every representative and
momentum-witness matrix. The counts are **family-local**, not a globally
independent basis: this first pass identifies 241 independent-tadpole products
with four existing representatives, one in each family. Other graph relations,
cross-family equivalences and fixed-index IBPs remain outside this pass.

Timings are preparation-only shared-host diagnostics, using optimized code,
CPU affinity 80–85 and nested pools limited to one. They exclude candidate
loading, reduction and the subsequent catalog comparison. Sources, optimized
binary/input hashes, complete alias records and process resource logs are in
`TMP/terminal-alias-census.P5ufji/`. These counts do not establish an application
speedup; paired first-target and cached-application measurements are separate.

### Paired saved-program application diagnostics

The first dotted target is `[2,1,1,1,1,1,1,1,1,0]`. Raw and normalized
applications ran in separate fresh processes using the same saved ordering and
the same optimized client. All returned coefficients agree exactly after an
independent Symbolica coalescing of the raw result; mass factors are unchanged.

| Input / mode | First application | Alias prepare + install | Load + prepare + first application | Output terms | Warm median, five lookups |
| --- | ---: | ---: | ---: | ---: | ---: |
| H, raw | 11.439 s | — | 12.510 s | 248 | 49 µs |
| H, normalized | 6.404 s | 0.334 s | 7.806 s | 189 | 34 µs |
| X, raw | 59.926 s | — | 61.940 s | 437 | 91 µs |
| X, normalized | 34.192 s | 0.424 s | 36.709 s | 357 | 72 µs |

Rule applications and cached integral counts are unchanged: 26,956 / 27,306
for H and 82,637 / 83,082 for X. The benefit is in coefficient accumulation:
H coalescing additions fall from 1,099,045 to 652,526, and X from 5,889,714 to
3,585,413. The cache's accounted coefficient bytes fall from 86,688,448 to
48,415,422 for H, and from 381,494,842 to 216,761,882 for X. These counters are
not total process memory. GNU whole-process peak RSS is 573,828 → 518,556 KiB
for H and 1,485,316 → 1,236,776 KiB for X.

These are **single-pair shared-host diagnostics**, not a confidence-bounded
speedup claim. Phase sums exclude file reading, printing and later comparison.
They are not whole-process timings or Vakint/FMFT timings. Five same-target
warm lookups all reproduce their respective first result exactly.

The control `[0,1,1,1,1,1,1,1,1,0]` is already a declared one-term terminal
in both programs and applies zero rules. Normalization offers no savings there:
the matched phase sums increase from 1.071 to 1.391 s for H and from 2.092 to
2.842 s for X, including load variability and alias preparation. Whole-family
preparation must be amortized over genuine reductions, not advertised as a
speedup for every call. All four control-process exact comparisons pass.

All eight application processes completed. Exact coefficients, workload and
binary identities, phase counters, process resources and reproduction commands
are retained in `TMP/terminal-alias-census.P5ufji/README.md` and its accompanying
logs. No rule generation, certification or FORM execution was part of this test.

## Remaining work

This product lane is a first reduction in redundant terminal keys, not the
complete terminal-reduction objective. General routed-graph equivalences,
factorized subproducts and bounded fixed-index terminal IBP relations remain
subsequent opportunities. Any exact subgraph transformation must carry its
Jacobian, normalization, domain and power/dimension mapping. In particular, a
fully massive bubble cannot be replaced by a single epsilon-shifted propagator.
See [the four-loop study](four_loop_terminal_deduplication.md) for the distinction
between raw keys, exact catalog projections and FMFT basis symbols.
