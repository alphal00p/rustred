# Rule generation and independent certification

## Workload boundary

SpIReD's `solver::solveSector` constructs reduction rules, follows exceptional
cases, and solves the remaining numerical cases. Its semi-numerical route also
checks reconstruction internally. It is therefore inaccurate to describe the
reference as doing no verification.

The supplied C++ vacuum examples do **not** run RustRed's additional independent
original-source replay, uniform descent/coverage validation, artifact installation,
or cold-loading validation. In `vendor/spired/examples/vac4.cpp`, the printed
sector timer includes calls to `solveSector` and writing each sector's `.rules`
and `.dat` files. It excludes preceding family preparation and the final
`MIs.dat` write. Reference-only source stays outside published RustRed content.

RustRed's existing `solver::SectorSolver` and `solver::SectorExecutor` likewise
allow generation without `SourcePortAudit`. The existing `family-solve` command
returns a diagnostic summary, not a reusable rule artifact. `family-close`
includes the independent publication pass and cannot be used as an equivalent
solver-only comparison.

## Split API

The application API exposes two separate operations:

1. Generate and save an explicitly **uncertified candidate bundle**, retaining
   the input family, ordering, sector/case definitions, exceptional conditions,
   exact formulas, finite residuals, and source-support provenance.
2. Certify that saved bundle without rerunning rule discovery. Reconstruct the
   family context, re-establish zero-sector evidence, replay original sources,
   validate descent and the complete cover, then emit a `ClosedArtifact` only
   on success. Cold loading is a separate measured operation.

Rust applications use `rustred_app::family_candidates` and
`rustred_app::certify_candidates`, with `FamilyCandidatesRequest` and
`CandidateCertificationRequest`. The CLI commands have the same names with
hyphens; public Python exposes `rustred.family_candidates` and
`rustred.certify_candidates`. Generation supports the generic source solver;
certification retains the current unit-mass-vacuum artifact admission.
See the [CLI workflow](CLI.md#save-candidates-certify-independently) and
[Python workflow](../crates/rustred-python/README.md#save-formulas-before-independent-certification).
The original `family-close` operation remains available with its original
combined semantics.

The saved candidate format is not a weaker spelling of `ClosedArtifact`. It
does not bypass admission in the production artifact loader. Unsupported
geometry, uncovered domains, or failed replay leave certification unsuccessful;
the reusable candidates remain available for diagnosis and experiments.

Experimental concrete application can check a requested integer point against
its actual case, all exceptional conjunctions, denominator nonvanishing, and
strict descent. It can stop at explicitly supplied finite residuals, but must
report a missing rule rather than invent another master. Such application checks
do not prove the formulas' IBP provenance or whole-family closure. Numerical
agreement with Vakint/FMFT is valuable additional evidence, not a replacement
for certification.

## Timing reports

Report separately:

| Boundary | Included work |
| --- | --- |
| Prepared-source solver | Sector discovery, exact rule construction, solver-internal checks |
| Solver plus output | Above plus equivalent rule serialization and file writes |
| Candidate certification | Decode/context preparation, source replay, descent, cover, installation and artifact encoding, with phase breakdowns |
| Cold validation/application | New-process artifact loading and a specified reduction |

Use release builds, identical input families, sector manifests, orderings,
backends and worker limits. State differences in output encoding rather than
assuming text and binary output cost the same. Include process-launch and
preparation time in a separate end-to-end measurement. Exclude compilation and
dependency downloads from solver comparisons. Never label an uncertified
generation time as the time to obtain an independently certified artifact.

The broader benchmarking protocol and earlier measured pairs are recorded in
[the parallel-port study](spired_parallel_results.md). Historical timings there
remain measurements of their specified revisions and workload boundaries, not
measurements of the new split API.

## First release validation

The split CLI reproduces the existing K1/K3/K6 artifact bytes, cold inspection
and canary reductions. K6 candidate bytes are identical at one, two and six
workers. Seven bundle tests, four CLI process tests, three new CLI parser
tests, and three new fresh-wheel Python tests exercise the split surfaces;
the broader core/frontend suites also pass. Evidence is retained at
`/tmp/rustred-split-candidates-release.qCn7z3/`.

A single serial K6 validation run measured 0.308 s of solving and 0.060 s of
bundle encoding. Independent certification took 1.687 s including decoding,
reconstruction and artifact encoding (the certification phase itself was
1.483 s). These are separate phases, not a paired performance study or a
SpIReD speed ratio.

The same CLI saved the full physical-root H and FG four-loop candidate sets:

| Input | Workers | Rules | Solve | Bundle encoding | Candidate bytes |
| --- | ---: | ---: | ---: | ---: | ---: |
| H | 2 | 21,360 | 51.458 s | 9.080 s | 240,832,048 |
| FG | 2 | 9,272 | 31.885 s | 3.145 s | 103,823,932 |

Both commands exited successfully, saving **uncertified** candidates. The
version-one text bundle retains source support and is substantially larger
than a rules-only output, so its I/O is not automatically comparable with
SpIReD's binary/text rule files. The reported solver phase excludes encoding;
full process times were 61.16 s and 35.33 s respectively. Compilation is
excluded. These shared-host single runs are validation observations, not
confidence intervals or head-to-head comparisons. The separate four-loop
certification blockers remain documented in the
[full-parent study](four_loop_parent_closure_probe.md).
