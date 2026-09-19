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
application diagnostics are recorded below. At this initial gate the hook was
not enabled in Vakint; the later combined routing rollout is recorded at the
end of this document. The core hook remains opt-in.

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

### Validated bounded extension: one dependence among active momenta

The opt-in `TerminalAliasPlan::vacuum_routing_equivalences` implementation considers nonnegative terminal
keys with exactly `L+1` active unit-mass propagators and momentum rank `L`.
The saved four-loop programs contain 433 nonnegative `L+1`-active-line
structural candidates: 328 with unit positive powers and 105 with raised
powers. The saved-program census below independently verifies momentum rank
`L` for every one of these candidates; structural counting alone did not
establish that fact.

There is exactly one linear dependence among those `L+1` momentum vectors.
Symbolica's exact matrix operations construct and replay its primitive integer
coefficients. Their absolute values, paired with the corresponding propagator
powers, provide a cheap proposal signature. Zero coefficients identify momenta
outside the dependent subset and must remain distinct from its members.

For example, the four momenta `k1, k2, k1-k2, k3` in a three-loop integral
have dependence coefficients `(-1, 1, 1, 0)`. The first three form a sunset
factor, and the last an independent tadpole. Changing `k1-k2` to `k1+k2`
changes the dependence signs but can be undone by reversing a loop momentum.
Raising the tadpole power is not interchangeable with raising a sunset power:
the zero/nonzero dependence coefficient is part of the signature. No subloop
integration, Gamma prefactor or epsilon-shifted propagator is needed here.

A matching signature is only a proposal, not proof of equal integrals. The
implementation tries a deterministic signed correspondence, reconstructs its
momentum map with native matrix operations, and requires exact active-denominator
replay, integral map entries, unit Jacobian, matching powers and no outstanding
symbolic conditions. A chosen momentum basis may itself have determinant other
than one; it is the **map between the two integrals** whose determinant matters.
Different lattice classes or tied correspondences can cause conservative misses.
They must retain their raw keys, never become assumed equalities.

Factored momentum vectors and the primitive circuit are prepared once per
active support and reused for dotted keys. The power-colored row ordering and
its basis inverse are still constructed for each key. This
slice avoids factorial permutation searches and leaves higher-rank dependence
spaces, numerator-bearing terminals and general graph proposal generation for
separate measured work. The existing product-only factory and default applier
behavior remain unchanged: installation of the combined plan is explicit.
Downstream Vakint activation is a separate numerical acceptance gate.

#### Saved-program census of the combined lanes

The release gate passed **57/57** tests: 28 terminal-normalization tests,
17 candidate-reducer tests, and 12 existing reducer tests. These include
independently authored adversarial cases, rank-deficient supports, distinct
lattice classes, bases with nonunit determinant but valid unit-Jacobian maps,
large integer shears, dots on independent versus dependent momenta, exact
ancestor accumulation, mass homogeneity, and memoization. A regression with
an asymmetric unused ISP verifies that active routing need not extend to a
full-family denominator permutation.

All four unchanged native programs completed the new census. Each first
preparation starts after native loading; four subsequent preparations compare
statistics, representative sets, and exact routing matrices. Only afterward
does the driver import the independently supplied native terminal catalog and
check every alias's exact projection with Symbolica. No oracle value enters
proposal construction or the momentum-map proof.

| Family | Raw keys | Product-only representatives | Combined representatives | New circuit aliases | First combined preparation (s) |
| --- | ---: | ---: | ---: | ---: | ---: |
| H | 386 | 312 | 176 | 136 | 0.911487 |
| FG | 145 | 106 | 53 | 53 | 0.367325 |
| BMW | 179 | 135 | 68 | 67 | 0.443762 |
| X | 445 | 365 | 208 | 157 | 1.009598 |
| Total | 1,155 | 918 | 505 | 413 | — |

The combined plan contains **650 verified aliases** (237 product aliases plus
413 circuit aliases), and all 650 independent catalog equalities pass. The
433 circuit candidates occupy 328 distinct active supports and retain 20
existing representatives. The 505 remaining keys comprise those 20, four
product representatives, 105 numerator-bearing keys, and 376 keys outside
the two active-line-count lanes. These are family-local finite representatives,
not a count or proof of independent masters.

All four repetitions per family produce identical representative and routing
matrix results. The census uses optimized libraries, CPU affinity 80–85 and
nested compute pools capped at one. It is a shared-host diagnostic, not a
six-worker scaling benchmark or a controlled timing distribution. No IBP rule
generation or closure certification is included. Reproduction evidence is
under `TMP/terminal-routing-census.ZU4xwc/`.

#### Fresh exact application comparison

The same new optimized client ran fresh raw/combined pairs for H and X with
`[2,1,1,1,1,1,1,1,1,0]`. Each owner starts with an empty cache and the same
persisted program and ordering. Plan preparation and installation are timed
separately. After the timers, every raw output coefficient is independently
coalesced through the verified representative map and compared exactly with
the combined reducer output. Family, target, common-mass degree, and five warm
cache results are also checked. Both dotted comparisons pass; the two pinch
comparisons below pass separately.

| Family | First apply raw → combined (s) | Preparation (s) | Load + prepare + install + first apply raw → combined (s) | Output terms raw → combined | Warm lookup median raw → combined (µs) |
| --- | ---: | ---: | ---: | ---: | ---: |
| H | 11.644410 → 4.185100 | 0.888323 | 12.763301 → 6.228792 | 248 → 91 | 53 → 20 |
| X | 61.796413 → 22.521394 | 1.012318 | 63.933806 → 25.663302 | 437 → 206 | 102 → 48 |

These single-pair diagnostics show 2.78×/2.74× apply-only improvements and
2.05×/2.49× improvements including the separately measured load and preparation
phases. They are not confidence bounds, matched FORM-backend comparisons, or
complete process-startup timings. File reading, output reporting and the
independent exact comparison are outside the phase sum. Earlier product-only
timings above remain historical evidence, not this pair's comparator.

| Family | Rule applications (unchanged) | Cached integrals (unchanged) | Coalescing additions raw → combined | Accounted cached coefficient bytes raw → combined | GNU whole-process peak RSS raw → combined (KiB) |
| --- | ---: | ---: | ---: | ---: | ---: |
| H | 26,956 | 27,306 | 1,099,045 → 432,343 | 86,688,448 → 31,262,384 | 571,348 → 490,908 |
| X | 82,637 | 83,082 | 5,889,714 → 2,268,880 | 381,494,842 → 131,889,422 | 1,481,572 → 1,103,744 |

The 63.94%/65.43% reductions concern accounted cache coefficient payload, not
RSS. Whole-process RSS also includes native program ownership, temporaries and
the post-timing exact comparison; it is distinct from `/proc/self/status`
snapshots printed inside the timed phases.

The matched D1-pinch control `[0,1,1,1,1,1,1,1,1,0]` is already a one-term
declared terminal and uses zero rules. It has no reduction speedup: matched
phase sums increase from 1.148540 to 1.970707 s (H) and 2.143164 to 3.134683 s
(X). Preparation must be amortized over actual reductions. Its four processes
also pass exact comparison and cache checks. All twelve census/application
processes exit successfully; commands, exact outputs, resources and ten
source/binary/input hashes are retained with the evidence README.

The remaining 376 keys outside these active-line-count lanes and 105
numerator-bearing keys are not normalized by this slice. An exact positive-power
parametric-equivalence proposal is documented separately in
[the next-lane study](vacuum_parametric_terminal_equivalence.md). Its implementation
and focused tests are separate; it is not counted in these routing results.

### Vakint routing-alias rollout

GammaLoop `vakint_rustred` commit `8e91d32f659dee237b5af04c6cd3383da2ee1585`
pins both RustRed dependencies to `f91c47abf820c9b9a376860b9c421675589b8a9c`.
The production four-loop loader explicitly prepares and installs the verified
routing plan before memoized application. The original constructor, other
backends, public defaults and raw terminal/catalog declarations are unchanged.
No rules or catalog values were regenerated or replaced.

An independent audit confirmed the unchanged 83 through-three-loop checks,
all 15 four-loop numerical references and all 16 expanded-numerator/pinch
comparisons. Nine focused constructor/catalog checks and three public fixture
checks also pass. The FORM-less lane uses FeynKit with invalid FORM paths;
FORM is used only by the separate oracle. The 15/16 matrices run inside two
test functions, not 31 separate Cargo tests. Workspace dependency generation
and CI-metadata checks passed, but this is not a claim of the entire CI suite.

The same optimized public benchmark ran in fresh processes before and after
activation: nine inputs, six calls per backend per input, 108 timed calls and
54 passing numerical comparisons per run. The same saved programs, ordering,
master values and scalar inputs were used. Affinity was 88–93, one scalar caller
and nested pools capped at one, on a shared AMD EPYC 9754 host. Other validation
work used disjoint cores; these single pairs are diagnostics, not confidence
bounds. Compilation, dependency-cache relocation and tensor preparation are
outside the scalar timing boundary.

| Public scalar observation | Before routing aliases | After routing aliases | FMFT after, same input |
| --- | ---: | ---: | ---: |
| H, cubed first propagator, first call (s) | 14.859 | 7.155 | 1.502 |
| X, cubed first propagator, first call (s) | 84.167 | 30.990 | 4.734 |
| H, same input, five-call warm median (ms) | 103.061 | 90.010 | 1490.813 |
| X, same input, five-call warm median (ms) | 150.270 | 126.704 | 4726.422 |

First-parent calls include lazy loading and plan preparation. Repeated calls
reuse process-local integral caches. Other input rows may also reuse previously
visited subproblems and are not cold-parent observations. The public backend
timer includes scalar application, master substitution, settings validation
and any internal dispatch; it is not a bare reducer-kernel measurement.
These dotted targets use power three, unlike the preceding core-only power-two
diagnostics. Numerical evaluation and comparisons lie outside
the public scalar timer.

The whole two-backend harness takes **164.83 → 96.35 s**, with user plus system
CPU **158.66 + 4.83 → 92.19 + 3.53 s**, and peak RSS
**2,864,780 → 2,218,696 KiB**. These include initialization and comparisons and
must not be presented as RustRed-only times or memory. First-use cubed parents
remain slower than FMFT; repeated calls are faster on all nine inputs. There
is no uniform speedup: FG's first cubed-parent call slightly worsens, and the
four-tadpole control does not benefit. All nine rows and the repeatable public
test command are in Vakint's `data/rustred/four_loop/README.md`.

Local evidence, including both immutable executable hashes, identical drivers,
raw observations, and independently checked runtime gates, is retained in
`TMP/gamma-terminal-alias-rollout.4Zi7ej/`. This activation uses momentum-routing
equalities only, not the new parameter-equivalence lane. It establishes finite
numerical acceptance, not master minimality or unrestricted family closure.
