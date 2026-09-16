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

## Split API under implementation

The application API is being extended with two separate operations:

1. Generate and save an explicitly **uncertified candidate bundle**, retaining
   the input family, ordering, sector/case definitions, exceptional conditions,
   exact formulas, finite residuals, and source-support provenance.
2. Certify that saved bundle without rerunning rule discovery. Reconstruct the
   family context, re-establish zero-sector evidence, replay original sources,
   validate descent and the complete cover, then emit a `ClosedArtifact` only
   on success. Cold loading is a separate measured operation.

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
