# Unrestricted Vakint native-package refresh

## Scope

This delivery follows the completed [restricted four-loop control](four_loop_saved_cover_control_2026-09-24.md).
It concerns Vakint's shipped **unrestricted** one- through four-loop rules,
not the bounded starting queries used by that control or the five-loop campaign.
The initial refresh re-exports existing IBPs without regenerating them. The
subsequent user-requested producer example also generates current rules afresh;
the two procedures and their measurements are distinguished below. Neither
procedure generates master values.

Both the older four-loop multi-sector packages and the saved-owner campaign
shards already use the same generic `RRPBIN` container, version 1, with Symbolica
native state and coefficient atoms. Different sector packing is not a different
file format. The re-export must preserve certified/candidate status, every rule,
terminal, ordering and scope. Bounded certified packages are not substitutes.

## Baseline and re-export

The baseline GammaLoop revision is `6b835ff0f8c654ac7b15cfa6d1e164209e46f580`,
with RustRed pinned to `8ad62b964de6f3a508fd165dc6ac2509f25839fe`.
The candidate pins RustRed and `rustred-app` to the published revision
`7b22f5bda441588f2e437b15fa0872461c483708`.

The refresh imports each trusted packaged coefficient table with the public
native reader, re-interns atoms in their original coefficient-ID order, and
writes through the current native writer. It requires identical coefficient IDs,
unchanged structural sections and equivalent native coefficient values/maps.
It neither regenerates rules nor upgrades their mathematical authority.
Byte-identical output leaves the original file and timestamp untouched; this is
a validated refresh, not a fictitious format migration. Four-loop packaging
retains compressed native programs and the existing 74-output master catalog.

The selected baseline numerical/timing executables embed their original assets
and cannot accidentally read candidate replacements. Release builds use the same toolchain and six
physical cores, CPUs 56–61. Compilation is excluded from scalar timing. These
tests run separately from the five-loop campaign's CPUs 0–49.

## Numerical and performance gates

The retained acceptance inventory comprises 83 selected lower-loop/regression
tests, the four-loop 15-input numerical reference gate, and the 16 additional
numerator/propagator cancellation tests. The RustRed/FeynKit path retains an
invalid FORM path. Only the MATAD/FMFT comparison oracle uses the vendored FORM.

Three additional ignored public-API timing tests cover one, two and three loops;
the existing nine-workload four-loop timing test remains in use. Baseline and
candidate use identical harnesses. Six process pairs alternate launch order.
First-family-use, including lazy loading, is reported separately from warm
memoized public calls. Every timed answer must pass its numerical comparison.
Per-process warm medians and paired ratios avoid treating repeated cached calls
as independent cold runs. A performance regression cannot be hidden by timing
only a favorable repeat.

## Current verification state

The baseline 83-test inventory passes in full. Its initial attempt failed because
the existing vendored FORM executable could not locate `libflint.so.24`, not
because of a numerical mismatch. An explicit runtime library path fixed the
oracle environment; both baseline and candidate must use that same environment.
The failed receipt remains part of the local record.

The dependency update required two test-only `SectorSolution` constructors to
specify `max_numerator_rank: None` and the default `SearchFinite` policy. All
terminal sets and assertions are unchanged; independent review confirms that
these remain unrestricted fixtures. The corrected release build passes.

All three lower-loop exports are byte-identical: K1 has 9 coefficients in
2,790 bytes; K3 has 56 in 36,692 bytes; K6 has 1,635 in 3,725,739 bytes. Their
six baseline/candidate process pairs per loop all pass, including 768 numerical
checks of first/warm native answers per loop. Independent audits reconcile every
pair and retain all measurements, including outliers.

First-family-use medians (milliseconds; scalar API boundary only):

| Loops / first input | Baseline | Candidate | Median paired candidate/baseline |
| --- | ---: | ---: | ---: |
| 1 / D4 | 115.369 | 114.473 | 0.9868 |
| 2 / D1 cubed | 125.474 | 122.090 | 0.9817 |
| 3 / D1 squared | 1,215.596 | 1,152.285 | 0.9492 |

Warm public-call medians (milliseconds): each process contributes its median
of 31 repeated calls; the table then reports medians across six processes.

| Input | Baseline | Candidate | Median paired candidate/baseline |
| --- | ---: | ---: | ---: |
| 1L / D4 | 46.938 | 47.381 | 1.0072 |
| 1L / D6 | 46.933 | 47.242 | 1.0032 |
| 2L / D1 cubed | 53.339 | 52.406 | 0.9788 |
| 2L / numerator | 49.312 | 50.351 | 0.9924 |
| 3L / D1 squared | 114.472 | 113.005 | 0.9924 |
| 3L / pinch 6 | 64.779 | 64.013 | 0.9833 |

The paired ratio need not equal the ratio of marginal medians. In particular,
the two-loop numerator's marginal median rises while its paired median falls;
both are shown rather than selecting a favorable summary. Most warm differences
have mixed signs across pairs. The three-loop D1-squared warm set retains one
1.2553 candidate/baseline outlier. These shared-host observations support broadly
comparable performance, not a universal speedup or a formal noninferiority claim.
The three-loop first-use improvement is consistent across all six pairs.

All four candidate exports now pass. Only the Symbolica-state section changes:
23 bytes in each 1,017-byte state. The coefficient-atom, family and program
sections are byte-identical, as are native file lengths and container version.
Compressed total size falls from 20,511,348 to 20,467,096 bytes (0.22%); this is
not a new format version, changed algebra or a speedup claim. Vakint is rebuilt
after replacing these embedded assets. The separate baseline four-loop run
passes all 15 reference and 16 cancellation cases; the runner explicitly clears
the optional family filter and verifies all 31 unique case markers.

The re-exported candidate passes all 83 selected regression tests and all
31 four-loop numerical cases. Its fixed six-pair four-loop timing matrix also
passes: all 12 processes and 648 numerical comparisons succeed. Across nine
workloads, median paired first-call candidate/baseline ratios are
0.9566–0.9856; warm ratios are 0.9808–1.0236. These support comparable
performance, not a uniform improvement.

## Reproducible fresh producers

The requested Python-steered producer example uses the same public generation
and native I/O services, followed by fresh-process loading and application or
terminal-normalization replay. Optional reference comparison is strictly
post-generation; imported programs are never source material for the solve.

Fresh K1/K3 match the shipped programs exactly in native structure and
coefficient meaning. Fresh K6 instead has 5,640 guarded cells against the
historical package's 5,639, with the same family identity, ordering, 38 terminals
and 26 zero sectors. This difference was already recorded in the September 19
native-format migration report; it is not a serialization difference. Both
certified files pass exact cold loading and the example canary. Fresh FG likewise
has 9,266 generated rules versus the historical 9,272, while retaining all 124
solved sectors, unrestricted rank and the same 16 normalized catalog keys.

Strict historical-reference comparisons correctly reject these differences.
The historical outputs and failed comparison receipts remain preserved. Fresh
production results must pass scope/catalog checks, Vakint's unchanged numerical
gates and new matched timings before replacing the packages. The re-export-only
timings above cannot stand in for those new-generation checks. This alignment
is in progress; it is not yet a completed delivery claim.

The full fresh producer run has now passed generation, cold inspection/application
at K1/K3/K6, and cold loading plus terminal-normalization replay and catalog-key
validation for every four-loop parent. No rank bound, ordering override or
authored rule source is supplied. Six native workers use CPUs 50–55; all nested
compute pools are capped at one. Whole generation-command wall times are:

| Family | Seconds | Generated rules | Raw terminals | Normalized catalog outputs |
| --- | ---: | ---: | ---: | ---: |
| K1 | 0.0043 | 1 | 1 | — |
| K3 | 0.0064 | 5 guarded cells | 2 | — |
| K6 | 1.4858 | 623 / 5,640 guarded cells | 38 | — |
| H | 19.3655 | 21,318 | 386 | 22 |
| FG | 25.8182 | 9,266 | 145 | 16 |
| BMW | 53.9676 | 9,018 | 179 | 17 |
| X | 101.2432 | 19,907 | 445 | 19 |

These include launch, generation and native encoding, not compilation,
subsequent cold checks, gzip packaging or terminal normalization. They are
single-run observations on a shared host, not a serial/parallel comparison or
a certificate-generation benchmark for the four-loop candidates. The four-loop
total is 59,509 generated rules across 900 nonzero sectors, with the unchanged
1,155 raw terminal keys normalizing onto the same 74 catalog outputs.

An independent public-API K6 audit cold-loads both certified snapshots. Of their
guarded-cell descriptors, 5,630 agree exactly. Nine old descriptors correspond
to ten new ones: one domain is split at `n0 = 0` versus `n0 <= -1` with unchanged
three-term RHS, and the remaining changes are redundant guard factors or
proportional guards. Thus the net extra cell does not introduce a new reduction
coefficient. The exact descriptor receipts remain in
`TMP/vakint-generation-example.P9si50/` alongside all generation records.

The preceding two-pass restricted four-loop descendant control used the
historical 59,636-rule snapshots. Its completion receipt must not silently be
relabelled as a complete finite-domain walk of these newly generated rules.
Conversely, the candidate packages retain their unrestricted input scope: they
are not the bounded control's queries or a certificate inferred from numerics.

A separate full invocation of the documented producer, now with strict
comparison to the newly installed packages, passes all seven comparisons.
Its generated payloads are byte-identical to the first complete fresh run;
historical-reference failure receipts are preserved separately. This repeat
took 284.195 seconds including generation, cold checks, normalization, packaging
and comparisons, excluding compilation. It is reproducibility evidence, not a
new universal benchmark claim.

The first full producer invocation took 276.182 seconds under the same six-core
resource envelope, with 685.062 seconds of waited-command CPU. Its sampled
aggregate peak RSS was 1.330 GB; the maximum waited-child `ru_maxrss` was
1,569,680 KiB. These are different resource measurements, not an asserted exact
aggregate peak. Both invocations finished without resource intervention.
Nine Python steering tests and the targeted Rust-helper format check pass.

Installation changes exactly five files: K6 and the four compressed candidate
programs. K1/K3, all four regenerated normalization sidecars and all four
precomputed value catalogs are byte-identical and left untouched. K6 is now
3,731,581 bytes; the four rule gzip files total 20,381,288 bytes. The
producer-aligned Vakint build passes; its numerical/performance gates are
separate from the intermediate re-export-only results above.
The final embedded packages now pass the unchanged 83-test regression inventory
and all 15 four-loop numerical references plus 16 propagator-cancellation cases,
with independent raw-marker and binary-binding audit. Their fixed six-pair
three-/four-loop timing matrices remain in progress; the old timing tables
are not substituted for those pending results.

Workspace-only raw build, export, numerical and timing evidence is retained in
`TMP/vakint-generic-refresh.uBYwrQ/`. Reference software, licenses, temporary
executables and raw campaign payloads are not RustRed repository content.
