# Four-loop generation comparison: RustRed and SpIReD

## Scope and timing boundary

This study measures candidate-rule generation, not independent certification or
an established four-loop closing artifact. It uses the four external unit-mass
inputs `examples/input/four_loop_{h,x,bmw,fg}.toml`, which correspond to Vakint's
four physical parents and its registered four-loop graph census. Every nonzero
subsector of each physical parent is requested; auxiliary scalar-product
coordinates stay nonpositive. The parent campaigns overlap, so summing them is
the cost of four complete campaigns, not a symmetry-deduplicated global basis.

Both implementations receive identical denominator orderings, natural integral
ordering, sixteen ordinary IBP sources, a numerical search depth of two, and
the same family-wide native zero-sector census. Symbolic case search is not
depth-limited. The initial baseline uses both default symbolic exact GPLU
backends. A second RustRed matrix also selects the existing Symbolica-backed
`SemiNumerical` option, at one and six workers, with the same inputs and solver
policy. It changes single-target symbolic lifting; direct seed hits, source
preparation and shared fixed-corner solves retain their normal paths. C++ keeps
its unchanged default exact backend.

The reconstruction controls match the existing example defaults: maximum degree
128, 200,000 probes, four attempts and eight primes. Each full-parent process has
a 900-second wall bound. A timeout or native reconstruction failure is reported
as such, not silently replaced by exact fallback or called a completed workload.

The supplied C++ `vac4` example is a different input and selects only 27
sectors. Its historical 72.67 s serial and 45.40 s six-worker process times must
not be used as the denominator for these complete-parent RustRed campaigns.

The temporary C++ driver reads external routing and sector files, resolves
ordering identifiers before parallel work, and calls the unchanged reference
solver. It retains results until solving ends and times serialization
separately. RustRed uses the public release `family-candidates` CLI and its
`solve_us` phase report. Neither measured solver phase includes rule-file or
candidate-bundle encoding, source preparation, RustRed's independent source
replay/closure certification, or cold loading. Solver-internal exact arithmetic,
case handling and applicability checks remain part of generation. C++ reads a
previously prepared native zero census, whereas RustRed computes it during
preparation; those unequal costs are excluded from the solver-phase comparison.
RustRed's normal active-first sector scheduling and C++'s dynamic manifest-order
scheduling are retained rather than imposing a different benchmark-only solver
policy on either implementation.

RustRed retains its documented correctness corrections to the reference,
including preservation of the full homogeneous numerical identity and explicit
activation guards. Matched input does not establish identical generated rules
or mathematical correctness of either candidate set. RustRed's text bundles also
retain source-support traces absent from the C++ rule files, so their output
times are not equivalent serialization workloads. The initial X CLI runs finish
search but fail candidate coefficient-byte admission during saving, leaving no
phase report. They therefore supply no measured X solve time. A temporary client
of the existing public Rust library measures generation separately from bundle
creation; it performs the same native family/zero preparation and sector solve,
retains all solutions across the solve interval, and writes native-display
formulas afterward. It also exposes the exact/reconstruction backend choice.
No production limit is bypassed and no closed artifact is produced by this
benchmark client. Counts and successful process exits are checked separately
from the still-pending certification.

## Initial measurements — matrix still in progress

The first CLI batch completed on 2026-09-16. These are RustRed sparse-exact
`solve_us` observations converted to seconds, excluding preparation and saving.
They are not yet a completed comparison against C++ or reconstruction.

| Parent | Requested nonzero sectors | Rules | Explicit residuals | One worker | Six workers |
| --- | ---: | ---: | ---: | ---: | ---: |
| H | 314 | 21,360 | 386 | 135.905 s | 33.466 s |
| X | 328 | Not recorded | Not recorded | Save admission failed | Save admission failed |
| BMW | 134 | 9,024 | 179 | 135.330 s | 60.143 s |
| FG | 124 | 9,272 | 145 | 68.413 s | 27.346 s |

All three successfully saved candidate bundles are byte-identical between one
and six workers. This is a reproducibility check, not an identity or closure
certificate. Both X searches reached saving, but exceeding the candidate
coefficient-byte limit prevented emission of a phase report. Their process
times must not be substituted for missing solve times. The generation-only
library client removes saving from the requested workload for the next matrix.

The matched library-client matrix includes all four parents, one and six
workers, and three configurations. Its completed or censored observations so
far are below. Pending jobs are not inferred from another frontend's times.

| Parent | Workers | C++ exact solve | RustRed sparse-exact solve | RustRed reconstruction solve |
| --- | ---: | ---: | ---: | ---: |
| H | 1 | Timed out at 900 s | 105.649 s | Prime budget exhausted; 185.90 s process |
| X | 1 | Timed out at 900 s | 299.662 s | Prime budget exhausted; 435.91 s process |
| BMW | 1 | Running | Pending | Pending |
| FG | 1 | Pending | Pending | Pending |
| H | 6 | Pending | Pending | Pending |
| X | 6 | Pending | Pending | Pending |
| BMW | 6 | Pending | Pending | Pending |
| FG | 6 | Pending | Pending | Pending |

The C++ H serial process used 900.06 s wall time and 890.11 s user CPU time,
then exited with timeout status 124 before producing any rule files. It has no
completed solver time; 900 s is not a speedup denominator. The RustRed H serial
library run used 107.19 s process wall time, of which 105.649 s was solving and
1.197 s was native-display output. Its full output census checks passed:
314 sectors, 21,360 rules, 386 residual records and 315 files. The matching CLI
counts corroborate workload selection, not certification. Different serial
observations on this shared host also demonstrate why one sample is not a
performance guarantee. No full-workload speed ratio is justified yet.

The H reconstruction run failed in sector `0000110110`, on a coordinate case
fixing indices 2 and 6 to zero and indices 4, 5, 7 and 8 to one (zero-based
positions; auxiliary index 9 is zero). Symbolica reported exhaustion of its
rational-reconstruction prime budget. The process exited 1 after 185.90 s;
there is no successful solve interval or completed output to compare. This is
an observed limitation of the selected eight-prime policy, not evidence that
reconstruction is mathematically impossible. Its 200,000-probe/four-attempt
controls apply per coefficient and prime, and its eight-prime allowance applies
per coefficient reconstruction, not globally to the family. Failed modular
images also consume that prime allowance, so this message alone does not
establish excessive final coefficient height. The implementation did not
silently switch to sparse exact arithmetic after this failure.

C++ X serial likewise reached the normal deadline: 900.07 s process wall time,
889.37 s user CPU time, exit 124, and no completed rule files. RustRed's matched
generation-only X attempt completed in 299.662 s solve time and 302.31 s process
wall time. Its full output check passed: 328 sectors, 19,980 rules and 445 finite
residual records. This successful phase report supplies the previously missing
X timing without changing or bypassing the app's candidate-bundle limits.
The X reconstruction run subsequently failed in the same sector and coordinate
case as H, again with Symbolica's prime-budget exhaustion error; its process
wall time was 435.91 s and exit status was 1. No completed reconstruction output
or solve-time comparison is available for either parent.

## Reproducibility

RustRed source: `725d756` (the split generation/certification API milestone).
The frozen optimized CLI SHA-256 is
`8cdee763d2c210c113a5fcd0470ec7c25873158cb14604b53016f857ea25b34a`.
RustRed uses its locked release build; C++ uses the existing optimized reference
library and an `-O3 -DNDEBUG -fopenmp` driver. Building and dependency setup are
outside all runtime measurements.

Both use CPU 38 for serial measurements and CPUs 38--43 for six workers. These
are distinct physical cores on one NUMA node of the shared AMD EPYC 9754 host.
They are not reserved or isolated from unrelated jobs. Nested worker pools are
capped. Single observations are not confidence intervals or a scaling guarantee.

Evidence directories:

- RustRed: `/tmp/rustred-four-loop-generation-comparison.prqBA3/`.
- C++ driver, census, logs and generated output:
  `/tmp/rustred-four-loop-cpp-matched.VRhy3D/`.

The runtime license is supplied only through the environment; it is not stored
in scripts or committed. Reference sources and generated rule data remain local.
