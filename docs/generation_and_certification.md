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

Rust callers can use `rustred_app::load_generated_candidate_bundle::<N>` to load saved
candidate formulas into the experimental `CandidateReducer` without repeating
sector search. The loader re-establishes family binding, zero-sector proofs,
root scope and coordinate priority. It does **not** authenticate source identities
or promote candidates to artifacts. `CandidateReducer::check_targets` reports
finite concrete reachability and the observed index bounds; it deliberately
returns a `CandidateReachabilityReport`, not a certificate.

For independent bounded-proof work, `SourcePortAudit::replay_sector_rule_batch`
can replay selected sector-local rule ordinals against original IBPs while
checking their declared guards. It omits whole-sector cover and does not certify
descent or reachable terminals. Joining this identity evidence with a concrete
reduction trace and persisting/enforcing a certified entry scope remains work
to do. The checkpoint does not ship a four-loop bounded or unrestricted artifact.

### Resumable candidate campaigns

`FamilyCandidatesRequest::checkpoint` accepts an optional
`CandidateCheckpointOptions::new(directory)`. Set `resume = true` explicitly
to reuse completed sector files. The CLI exposes `--checkpoint-dir`, `--resume`
and `--checkpoint-max-bytes`; Python exposes the matching keyword arguments.
No core solver or algebra strategy is changed, and the mechanism is independent
of topology and loop count. See [the operational contract](CLI.md#save-candidates-certify-independently).

Completed sectors are atomically saved as ordinary native candidate bundles.
Only small storage receipts remain in the executor; expanded exact solutions
are released after writing. A manifest binds the exact campaign, but worker
count and caller resource limits may change on resume. Assembly rebuilds the
same logical program in original order, admitting cumulative structural and
coefficient limits. It does not retain every decoded shard simultaneously.
Native family and indexed coefficient contexts are checked on assembly;
structural request admission precedes native import. This is restart support,
not source replay or authority for missing sectors.

The complete-resume path performs preparation/assembly only. Reports distinguish
reused work and newly solved sectors; previous solve time is not fabricated.
The optional disk-payload budget includes reservations and abandoned temporary
files, but is not a filesystem-block or RSS cap. Active workers and final global
native output can still exhaust resources. No saved files are automatically
removed, overwritten on mismatch, or treated as a successful final bundle.

The September 20 release gate passes 114 application unit tests, 72 integration
tests and 38 fresh-extension Python tests. Small-family tests compare native
structure, exact coefficients and both ordered variable maps across ordinary,
serial-checkpoint, fresh six-worker checkpoint and resumed output. Fresh CLI
processes independently certify and apply the resumed output. Store tests cover
locking, failed/ambiguous publication, retained temporary files, mismatch and
disk limits; integration tests cover partial original-ordinal scheduling and
final-output-limit failure followed by assembly-only retry. These gates do not
establish a peak-memory improvement or any new family closure. Raw evidence:
`TMP/candidate-checkpoint-release-fixed.Fkhysy/`.

### Checkpoint K6 release smoke

The unchanged external `examples/input/three_loop_k6.toml`, natural full root,
`sparse-factorized`, numerical depth two and default final-output limits pass
four fresh-process generation modes. Each saves 623 rules and 38 finite residuals
across 38 nonzero sectors, with 26 zero sectors and 1,417 native coefficients.
Candidate outputs are 335,561 bytes each. Exact structural/coefficient/ordered-map
comparison passes for every mode; ambient State bytes are not the identity.

| Mode | Workers requested | Solve + sector-save (s) | Final encoding incl. assembly (s) | Application total (s) | Process CPU (s) | Peak RSS (KiB) |
|---|---:|---:|---:|---:|---:|---:|
| Ordinary, no checkpoints | 1 | 0.344661 | 0.010735 | 0.365647 | 0.36 | 12,288 |
| Fresh checkpoints | 1 | 0.385074 | 0.060962 | 0.450522 | 0.43 | 12,340 |
| Fresh checkpoints | 6 | 0.101717 | 0.061130 | 0.167381 | 0.52 | 12,352 |
| Complete resume of serial files | 6 | 0.000028 | 0.060954 | 0.068347 | 0.07 | 12,312 |

These are single release-profile observations, not a scaling confidence interval
or a five-loop memory result. Preparation and checkpoint admission account for
the remainder of application total. Whole-process wall times, including launch,
final file writes and shutdown, are respectively 0.37, 0.46, 0.17 and 0.08 s;
the CPU/RSS columns use that whole-process boundary. The last row performs **no search or worker
pool construction**: its tiny solve field is bookkeeping, not a new solve.
Serial native assembly is about 58 ms and is already inside final encoding,
not an extra phase to add twice. Checkpoint encoding/filesystem I/O is inside
the solve field for fresh saves. Each checkpoint directory retains 688,675
logical bytes, and complete resume leaves every managed file byte-identical.

Every mode then independently certifies to an equivalent V6 artifact (5,640
rule cells, 38 terminals, 3,731,581 bytes) and cold-reduces `[2,1,1,1,1,1]`.
Certification and cold load/reduction are **not included** in the generation
table. All four decompositions agree, and all 14 harness stages exit zero.
CPUs 88–93, nested pools one, per-stage 60-second deadline and 8 GiB virtual
limit were fixed beforehand. Compilation is outside every measured boundary.
Evidence and separate process wall/CPU/RSS reports are retained in
`TMP/k6-checkpoint-fixed.ALZUbh/`.

The first smoke in `TMP/k6-checkpoint-smoke.7148XT/` found an inherited CLI
atomic-output bug: a bare filename supplied an empty parent path to directory
sync. It installed a candidate file but returned exit 7 without a report, so
no checkpoint or certification stages ran. The shared writer now treats that
parent as `.`. Isolated subprocess regressions and the fresh smoke retain the
same basename paths; the failure was fixed, not bypassed or relabeled success.

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
