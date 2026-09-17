# Four-loop generation comparison: RustRed and SpIReD

The [September 17 status report](../STATUS_17_09_2026.md) contains the compact
combined comparison and current delivery status. The bounded attempts below
are historical; the later uncapped and reconstruction sections contain the
final checkpoint outcomes. No benchmark remains running at this checkpoint.

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
128, 200,000 probes, four attempts and eight primes. The initial matrix used
a 900-second wall bound per process. The reconstruction follow-up below retains
the same solver limits and lifts the external deadline for its BMW/X rows.
A timeout or native reconstruction failure is reported
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

## Initial bounded measurements

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
| BMW | 1 | Timed out at 900 s | 116.480 s | Prime budget exhausted; 162.89 s process |
| FG | 1 | Incomplete terminal record | 62.305 s | Prime budget exhausted; 72.42 s process |
| H | 6 | Timed out at 900 s | 33.305 s | Prime budget exhausted; 42.69 s process |
| X | 6 | Timed out at 900 s | 103.340 s | Prime budget exhausted; 104.61 s process |
| BMW | 6 | Timed out at 900 s | 64.690 s | Prime budget exhausted; 25.77 s process |
| FG | 6 | Not recorded | 29.251 s | Prime budget exhausted; 20.10 s process |

The initial C++ FG1 stdout contains a 446.808 s solver report, but its external
timing file is empty, so no terminal process/resource record is available for
that attempt. It is excluded from the accepted uncapped completion table below.
No initial FG6 terminal record is available. These missing records are not
reclassified as timeouts or successful process measurements.

The C++ H serial process used 900.06 s wall time and 890.11 s user CPU time,
then exited with timeout status 124 before producing any rule files. It has no
completed solver time; 900 s is not a speedup denominator. The RustRed H serial
library run used 107.19 s process wall time, of which 105.649 s was solving and
1.197 s was native-display output. Its full output census checks passed:
314 sectors, 21,360 rules, 386 residual records and 315 files. The matching CLI
counts corroborate workload selection, not certification. Different serial
observations on this shared host also demonstrate why one sample is not a
performance guarantee. No completed C++/RustRed full-workload speed ratio is
justified yet.

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

C++ BMW serial also timed out normally after 900.06 s wall time (889.97 s user
CPU, exit 124), without completed rule output. To obtain parallel observations
earlier, the pending queue now runs H6 and X6 before finishing the remaining
BMW/FG jobs. Only the supervisor was changed after BMW's child exited; no timed
job, input, binary, policy, timeout or affinity was changed or rerun.

The H six-worker C++ process also exceeded its deadline (900.14 s wall,
1,618.92 s user CPU, exit 124). RustRed sparse exact completed the identical
requested sector set in 33.305 s solver time (34.82 s whole process, 163.93 s
user CPU, 522,264 KiB peak RSS). All 315 native-display/count files are byte-for-
byte identical to the serial RustRed output. The observed RustRed serial-to-six
solver speedup is 3.17×; this is one paired observation, not a confidence bound
or a C++ speed ratio. Six-worker reconstruction failed on the same case and
native prime-budget error after 42.69 s process wall time, so it still supplies
no successful generation timing or output.

C++ X with six workers also reached the deadline: 900.34 s process wall,
4,116.39 s user CPU, 2,958,856 KiB peak RSS, exit124, without completed output.
After that normal child boundary, the untouched Rust BMW/FG backend rows were
moved ahead of the remaining C++ deadlines to obtain the requested Rust
reconstruction/parallel comparison sooner. No timed job or frozen configuration
was modified; all remaining C++ observations are still scheduled.
RustRed sparse exact X then completed in 103.340 s solver time (106.17 s
whole process, 306.16 s user CPU, 964,540 KiB peak RSS), with all 329 complete
native-display/count files byte-identical to its serial output. Its observed
serial-to-six solver speedup is 2.90×, subject to the same single-observation
shared-host qualification as H.
X six-worker reconstruction also failed with the same coordinate-case/native
prime-budget error (104.61 s process wall time, exit1), with no completed output
or successful solver interval.

All sixteen RustRed matrix jobs are now terminal. BMW sparse-exact solving took
116.480 s with one worker and 64.690 s with six; FG took 62.305 s and 29.251 s.
Their process times were respectively 117.70/65.62 s and 62.80/29.77 s. Full
output checks passed for every successful run, including exact requested sector
censuses and aggregate rule/residual counts. All native-display/count files are
byte-identical between one and six workers for every parent: H 315 files,
X 329, BMW 135, and FG 125. The observed BMW and FG solver speedups are 1.80×
and 2.13×. These comparisons establish deterministic displayed candidate
outputs, not replay certification, universal coverage, or equality to C++.

The two BMW reconstruction attempts failed in sector `0001011100`, on the
coordinate case with indices 3, 5, 6 and 7 fixed to one, indices 2, 8 and 9
fixed to zero, and indices 0, 1 and 4 free. Both FG attempts failed in sector
`1011100100`, with indices 0, 2, 3, 4 and 7 fixed to one, indices 5, 6 and 9
fixed to zero, and indices 1 and 8 free. Each pair selected the same failing
case and reported the same native prime-budget exhaustion. Thus the initial
reconstruction matrix has no completed full-parent output; its shorter parallel
failure times must not be presented as successful generation speedups.

The original capped observations above remain historical evidence. Separate
uncapped completion attempts now provide the following full-parent results.

## Uncapped C++ completion and output comparison

The accepted uncapped rows use the unchanged optimized `driver` and reference
library, with the approved pre-resolution of ordering identifiers. There is
no external 900-second deadline. They retain the same input manifests, sixteen
ordinary sources, numerical depth two and prepared-source solver boundary.
The original driver does not emit sector progress; CPU/RSS samples establish
resource behavior, not completed sector counts before serialization.

| Parent | Workers | C++ solve | Process wall | CPU user + system | Peak RSS (KiB) | Result |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| H | 1 | 3,182.388 s | 3,186.76 s | 3,152.94 s | 920,452 | Exit 0; complete output checked |
| H | 6 | 1,987.631 s | 1,993.00 s | 3,612.00 s | 1,414,728 | Exit 0; complete output checked |
| FG | 1 | 2,234.175 s | 2,245.68 s | 1,686.74 s | 310,480 | Exit 0; complete output checked |
| FG | 6 | 278.706 s | 280.37 s | 414.19 s | 521,304 | Exit 0; complete output checked |
| BMW | 1 | 24,390.850 s | 24,401.14 s | 23,808.82 s | 4,533,264 | Exit 0; complete output checked |
| BMW | 6 | Checkpoint-censored | 3,005.99 s | 5,251.41 s | 2,189,212 | Checkpoint stop (SIGTERM); no completed solve time |
| X | 1 | Not started | — | — | — | Uncapped run deferred for requested checkpoint |
| X | 6 | Not started | — | — | — | Uncapped run deferred for requested checkpoint |

Serial runs use CPU 38 and six-worker runs CPUs 38–43. The launcher sets both
`OMP_NUM_THREADS` and `OMP_THREAD_LIMIT` to the requested count, disables dynamic
teams and nested active levels, and caps BLAS pools at one. Recorded BMW6 samples
verify six OS threads and affinity 38–43 for every thread. An earlier interrupted
H6 attempt is excluded from this table. Shared-host contention is uncontrolled:
FG's apparent superlinear ratio and differing aggregate CPU times must not be
interpreted as a scaling guarantee.

Complete saved-output checks independently count actual arrows in every C++
text rule file, inspect every nonempty binary rule file, check the expected file
set and compare all requested masks with RustRed and the input manifest. H's
631 and FG's 251 files are byte-identical between serial and six-worker runs.
This includes binary/text rules, the family archive, master list and per-sector
counts. Output serialization remains outside each quoted solver interval.

| Parent | Sectors | RustRed rules | C++ rules | RustRed residual keys | C++ residual keys | Sectors with differing rule/residual counts |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| H | 314 | 21,360 | 21,297 | 386 | 442 | 10 |
| FG | 124 | 9,272 | 9,224 | 145 | 188 | 7 |
| BMW | 134 | 9,024 | 8,957 | 179 | 250 | 11 |

For each completed parent, every RustRed residual key is present in the C++
set. C++ has respectively 56, 43 and 71 additional keys. These are explicit
set comparisons, not an inference from counts. The generated rule collections
are therefore different; candidate counts or matched sectors do not establish
mathematical equivalence. RustRed retains its intentional source-translation
and activation corrections. Native application canaries compare concrete
reductions separately from this structural census.

Reproducible local evidence is in
`TMP/rustred-four-loop-cpp-matched.VRhy3D/`: the accepted
`cpp-h-w1-uncapped`, `cpp-h-w6-uncapped-valid`,
`cpp-fg-w{1,6}-uncapped-valid` and `cpp-bmw-w1-uncapped-valid` prefixes;
`check-cross-output.py` and `cross-output-census-20260917.json` retain the full
per-sector and master-set differences. `run-remaining-uncapped.sh` and BMW6's
`.resources` record its launcher and resource samples. On the user's checkpoint
request, only the waiting supervisor was initially paused to prevent queued X
jobs from starting. Its BMW6 child continued unchanged until checkpoint
preparation, then received SIGTERM at 08:25:02 UTC on 2026-09-17. No rule files
or completed solver report had been serialized. All owned processes and the
paused supervisor were stopped afterward. GNU time's `exit_code=0` field for
this signaled process is not a success: its explicit `Command terminated by
signal 15` record and absent completed output establish the censor.

## Native exact application canaries

Concrete reductions are checked separately from the structural output census.
The C++ canary cold-loads its native family/rule databases and calls the existing
`topDownReducer`; every returned key must belong to its declared residual set.
The Rust canary cold-loads the saved candidate bundle with the public
`load_candidate_bundle` API and calls the native `CandidateReducer`, clearing
its cache before each direct target reduction. No candidate search is repeated.

Because the two residual sets differ, each returned C++ key is further reduced
with RustRed's native candidate applier. Native checked Symbolica arithmetic
then combines the transported coefficients and subtracts the independent Rust
target result over their full union of keys. Zero differences establish exact
agreement for these integer inputs at generic symbolic `d`; this does not
certify every generated rule or the extra Rust rules used for basis transport.

| Parent | Completed targets | Main recurrence target | New Rust rule applications | Rust result terms | C++ result terms | Exact agreement after basis transport |
| --- | ---: | --- | ---: | ---: | ---: | --- |
| H | 4 | First denominator squared | 26,956 | 248 | 285 | All coefficients equal |
| FG | 3 | First denominator squared | 3,362 | 78 | 99 | All coefficients equal |
| BMW | 5 | First denominator cubed | 16,113 | 179 | 217 | All coefficients equal |

H's four targets are its parent, the squared-denominator parent, a factorized
dotted subsector and the third-denominator pinch. The main recurrence returned
40 C++ keys outside RustRed's declared residual set; native transport produced
248 terms and an identically zero exact difference. The factorized dot applied
one new rule and also agreed; parent and pinch are declared-terminal controls.
FG's parent, squared-denominator parent and third-denominator pinch all agree
as well. Its main recurrence transports 21 additional C++ residual keys into
the 78-term Rust result, again with zero exact coefficient differences.
BMW's five targets also agree: parent, first denominator squared and cubed,
factorized dot and third-denominator pinch. Squaring the first BMW denominator
is already a declared-terminal control; cubing it exercises 16,113 applications.
Its C++ result contains 42 additional residual keys; native transport yields
179 terms and zero exact differences. The factorized dot applies one new rule.
All twelve concrete target comparisons completed with successful process exits.

The temporary comparison harness was independently source-reviewed. Its Rust
client is optimized but links the checkpoint's debug application/core libraries,
so its application and reload times are correctness diagnostics and are not
compared with optimized C++ performance. Reproducible local evidence uses the
`cpp-native-*-canary` and `rust-cross-*-canary` prefixes, the explicit
`canary-*-targets.tsv` files, `rust-cross-canary.rs` and
`NATIVE_CROSS_CANARY_PROTOCOL.md` in the same ignored evidence directory.
`check-native-canaries.py` and `native-canaries-final-checked.json` verify all
twelve successful terminal records, exact-zero difference files and complete
native output sets. The transported and direct native displays are also
byte-identical for every target.

## Reconstruction follow-up with frozen pivot2 binary

The follow-up uses the optimized `rust-backend-client-pivot2`, SHA-256
`bb5230a72be10609c52ae80913f7304772b84bacdcfd9f2b35e8d2064c7d3168`,
with the same compact inputs, ordered sector lists, solver controls and
one-/six-worker matrix. These are measurements of that frozen binary, which
predates the later chronology and RNG fixes; they must not be attributed to
subsequent source revisions. No build is included in a runtime measurement.
All eight rows completed successfully; the final serial X run finished on
2026-09-17 before the requested checkpoint.

The repaired reconstruction path includes exact sparse source-support replay
and support retries within generation. That work remains inside `solve_us`.
Symbolica owns the interpolation and rational reconstruction. Internal replay
does not replace the independent guard, descent, coverage, artifact-encoding
and cold-reload certification required for a closing artifact.
The solve columns exclude preparation and output; wall time, CPU time and
peak RSS describe the complete reconstruction process.

| Parent | Workers | Sparse-exact solve | Reconstruction solve | Reconstruction process wall | Reconstruction CPU user + system | Reconstruction peak RSS (KiB) | Result |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| H | 1 | 105.649 s | 541.876 s | 551.91 s | 472.88 s | 357,980 | Exit 0; all files checked |
| H | 6 | 33.305 s | 65.878 s | 67.12 s | 302.62 s | 487,348 | Exit 0; all files checked |
| FG | 1 | 62.305 s | 191.468 s | 193.10 s | 191.04 s | 164,920 | Exit 0; all files checked |
| FG | 6 | 29.251 s | 71.996 s | 73.31 s | 249.82 s | 271,776 | Exit 0; all files checked |
| BMW | 1 | 116.480 s | 383.279 s | 384.04 s | 356.21 s | 249,296 | Exit 0; all files checked |
| BMW | 6 | 64.690 s | 395.940 s | 402.21 s | 1,131.26 s | 429,820 | Retry exit 0; all files checked |
| X | 1 | 299.662 s | 894.666 s | 898.85 s | 889.48 s | 689,544 | Exit 0; all files checked |
| X | 6 | 103.340 s | 593.857 s | 598.56 s | 3,148.72 s | 905,900 | Exit 0; all files checked |

Both H rows contain 314 sectors, 21,360 rules, 386 finite residual records and
315 files. Both FG rows contain 124 sectors, 9,272 rules, 145 residual records
and 125 files. The complete ordered sector manifests, exact output file sets,
aggregate counts and actual rule/residual records in each file were checked.
Every output file is byte-identical to its matching sparse-exact output and
to the other reconstruction worker count. This establishes matching displayed
candidates, conditions and residuals; it is not a closure certificate.
Both BMW rows likewise passed their complete 134-sector, 9,024-rule,
179-residual, 135-file checks, with every file byte-identical to sparse exact
and to the other reconstruction worker count. The backend controls, sixteen
ordinary sources, global zero census and complete family fingerprint also
match the corresponding sparse runs.
Both X rows also passed their complete 328-sector, 19,980-rule,
445-residual, 329-file checks, with every file byte-identical to sparse exact
and to the other reconstruction worker count. Thus all eight completed
outputs have been checked in full, including their 1,808 output files.
An independent audit reran all eight completed rows' validator and separately
asserted that every comparison flag was true, with no missing, extra or
differing files. It also reviewed the timing boundary and frozen-binary
attribution; its evidence is `PIVOT2_CPP_LANE_INDEPENDENT_AUDIT.md` and
`pivot2-independent-final-cpp-lane-audit.json` in the local evidence directory.

Reconstruction is slower than sparse exact in each completed observed pair.
BMW reconstruction also takes longer with six workers than with one, and uses
substantially more aggregate CPU time. The new FG/BMW/X rows use CPU 44 for
serial and CPUs 44–49 for
six workers, with nested worker pools capped at one; the historical sparse
rows used CPU 38 and CPUs 38–43. These are shared-host samples, not controlled
confidence bounds. In particular, H's apparent superlinear serial-to-six
ratio is not a scaling guarantee; its aggregate CPU times also differ.
BMW serial also overlaps local application-canary work on CPU 44 for about
43 seconds and one earlier 0.29-second check; that contention remains in its
solver/process wall observation.

FG1 and FG6 completed under their original 900 s deadlines. The later BMW/X
children start without an external wall deadline, retaining the same eight-
prime and other solver limits. The FG child retained its existing command
through the deadline-policy change.

The first new BMW six-worker attempt was interrupted during later controller
cleanup. Its last process sample showed 82 s elapsed, about 305 s aggregate
CPU time and 403,120 KiB current RSS; there is no terminal timing report or
generated output. This is a controller interruption, not a reconstruction
failure or a completed solve time. The separately named `w6-retry` attempt
uses the same frozen binary and workload under an uninterrupted controller.

Reproducible local evidence is under
`TMP/rustred-four-loop-cpp-matched.VRhy3D/`: per-row `.stdout`, `.stderr` and
`.time` files, complete output directories, `check-pivot2-matrix.py`,
`pivot2-matrix-final-checked.json`, `run-pivot2-remaining.sh` and
`PIVOT2_MATRIX_PROTOCOL.md`. Runtime licenses and generated reference data
are not committed.

## Reproducibility

For the initial matrix, RustRed source was `725d756` (the split
generation/certification API milestone).
The frozen optimized CLI SHA-256 is
`8cdee763d2c210c113a5fcd0470ec7c25873158cb14604b53016f857ea25b34a`.
RustRed uses its locked release build; C++ uses the existing optimized reference
library and an `-O3 -DNDEBUG -fopenmp` driver. Building and dependency setup are
outside all runtime measurements.

The initial matrix used CPU 38 for serial measurements and CPUs 38--43 for
six workers. These
are distinct physical cores on one NUMA node of the shared AMD EPYC 9754 host.
They are not reserved or isolated from unrelated jobs. Nested worker pools are
capped. Single observations are not confidence intervals or a scaling guarantee.

Evidence directories:

- RustRed: `/tmp/rustred-four-loop-generation-comparison.prqBA3/`.
- C++ driver, census, logs and generated output:
  `/tmp/rustred-four-loop-cpp-matched.VRhy3D/`.

The runtime license is supplied only through the environment; it is not stored
in scripts or committed. Reference sources and generated rule data remain local.
