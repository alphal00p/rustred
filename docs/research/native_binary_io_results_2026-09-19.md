# Native program I/O: implementation and measured migration

## Scope and result

The four saved four-loop candidate programs were converted without generating
any new IBPs. Exhaustive same-process and separate fresh-process comparisons
preserved all 59,636 rules, 1,155 declared residual keys and 1,579,493 coefficient
uses. The new programs contain native Symbolica rational-polynomial atoms,
their shared state context, native family geometry and structural rule records.

The candidate-format implementation and its saved-data migration are pushed.
The subsequent certified V6 migration, described below, preserves the existing
proof records while moving their algebra into the same native envelope and
coefficient table. Its final validation and atomic downstream asset rollout
also pass and are pushed as RustRed `d6718733` and Vakint `39992f757`.
Transport equivalence and successful concrete reductions do
not certify unrestricted family closure.

## What changed

`rustred::persistence` supplies a topology-independent, versioned section
envelope, a first-occurrence coefficient dictionary and `NativeFamilyRecord`.
The dictionary uses Symbolica's native packed rational-polynomial `Atom` and
`State` serialization. It avoids printed coefficient expressions, reparsing
those expressions, or adding a RustRed algebra kernel. Equal constants on
different base/indexed variable maps deliberately remain distinct entries.

Family dimension, affine denominator matrix and constants, external Gram
matrix and power shifts use the same native dictionary. Momentum/parameter
labels are structural metadata. Original input text remains provenance only:
loading reconstructs the family through its ordinary exact constructor and
checks its fingerprint, without reparsing that text. The binary transport has
no loop/topology or 16-denominator restriction; the existing application
dispatch retains its separate current arity admission.

The public application seam is `load_generated_candidate_bundle`; lightweight
`inspect_generated_candidate_bundle` reads structural metadata without importing
Symbolica state or coefficients. The payload schema is
`rustred.generated-candidates.binary.v1`. Generation and optional subsequent
certification remain separate operations. The old TOML candidate loader is not
retained as a production compatibility path.

Native imports explicitly require trusted generated data and a matching
Symbolica stack. Outer sizes, frames, counts and exact consumption are checked,
but Symbolica's native readers are not hardened hostile-input parsers. The fast
path preserves the producer's normalized coefficients rather than reproving
coprimality. A separately selectable native import normalizes each unique entry
once using Symbolica. No hot-path authentication or new CAS implementation is
introduced. The initial format supports the native 64-bit layout; byte identity
across different ambient Symbolica states is not promised.

## Saved-program equivalence and sizes

All sizes below are bytes. Gzip comparisons use exactly `gzip -n -9`: the
independent audit freshly recompressed all old inputs and obtained byte-identical
copies of the shipped files, then compressed the new inputs using the same
command and checked decompression against the native originals.

| Program | Rules | Residual keys | Old raw | Native raw | Old gzip -9 | Native gzip -9 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| H | 21,360 | 386 | 240,831,356 | 58,178,184 | 6,757,903 | 4,678,555 |
| FG | 9,272 | 145 | 103,823,374 | 22,157,549 | 2,849,864 | 1,953,460 |
| BMW | 9,024 | 179 | 189,143,178 | 50,164,550 | 5,296,341 | 3,624,123 |
| X | 19,980 | 445 | 537,049,880 | 150,237,974 | 15,042,937 | 10,255,210 |
| Total | 59,636 | 1,155 | 1,070,847,788 | 280,738,257 | 29,947,045 | 20,511,348 |

The raw total is 73.8% smaller; the level-9 compressed total is 31.5% smaller.
A second same-turn comparison freshly compressed both old and new inputs using
`gzip -n -6`: totals were 34,218,772 and 22,432,435 bytes respectively. Do not mix
those level-6 sizes with the shipped level-9 baseline.

The dictionaries contain 87,708 / 33,422 / 71,988 / 203,949 entries for
H / FG / BMW / X. Each shared Symbolica state section is just 1,017 bytes in
these processes. Ambient-state export is therefore not the dominant storage
cost for these four programs; this does not imply native state is always
strictly restricted to reachable resources.

The offline verifier compared every source row, source integral/translation,
target, coordinate or affine case, ordered exceptional conjunction, priority
permutation and finite residual. It independently reparsed each distinct old
expression using Symbolica and compared every native coefficient occurrence
and its polynomial-versus-rational role. Family geometry and fingerprint were
checked separately. Every conversion and fresh verification exited zero.

## Cold-load and application boundary

Measurements use optimized release libraries, fresh processes, CPU affinity
88–93 and nested pools limited to one. Raw file reading is recorded separately;
the load timer starts with already-read bytes and ends with the ordinary
`CandidateReducer` constructed. It includes native decoding, family/zero-sector
preparation and applier construction, but excludes filesystem read, gzip
decompression and Vakint matching. The old consumer obtains the same integer
target from Vakint's routing witness before timing; the new consumer receives
that target directly and checks the family binding. Neither searches for rules
nor invokes FORM.

These are single-run shared-host diagnostics while compilation and an uncapped
C++ experiment were also running. They are not an isolated paired statistical
benchmark. Both consumers apply the target `[3,1,1,1,1,1,1,1,1,0]`; the new
consumer compares all resulting terminal coefficients exactly against the
frozen old output using Symbolica after all timed operations.

| H diagnostic | Old text program | Native program |
| --- | ---: | ---: |
| Decode and construct reducer, wall | 22.491201 s | 1.074107 s |
| Decode and construct reducer, CPU | 22.28 s | 1.06 s |
| First application, wall | 16.022401 s | 13.650023 s |
| First application, CPU | 15.86 s | 13.51 s |
| Median of five cache-hit applications | 68 µs | 72 µs |
| Whole-process peak RSS | 6,988,800 KiB | 585,732 KiB |

The H canary passed exact equality for all 340 terminal coefficients. Both
executions made 27,496 rule applications, 5,586 cache hits and 1,235,725
coalescing operations. The native post-load resident snapshot was 488,928 KiB;
whole-process peak includes later application, output and comparison work.
The applier algorithm did not change. The first-application timing difference
must not be presented as evidence of a new solver or arithmetic backend.

| X diagnostic | Old text program | Native program |
| --- | ---: | ---: |
| Decode and construct reducer, wall | 45.555304 s | 2.104254 s |
| Decode and construct reducer, CPU | 45.18 s | 2.08 s |
| First application, wall | 82.125801 s | 80.068346 s |
| First application, CPU | 81.39 s | 79.50 s |
| Median of five cache-hit applications | 74 µs | 96 µs |
| Whole-process peak RSS | 12,954,700 KiB | 1,649,700 KiB |

X also passed exact equality for all 437 output terminal coefficients and all
five cache repeats. Both executions made 88,134 rule applications, 15,065 cache
hits and 7,592,596 coalescing operations; both retained 88,579 cached integrals
and 10,264,920 coefficient terms. Native post-load resident size was
1,021,348 KiB, rising to 1,651,172 KiB after application according to the
process-status snapshot. These snapshots and GNU-time peak RSS use different
kernel accounting interfaces; the table reports GNU time consistently.

Thus these diagnostic load phases were about 21 times faster, while the
unchanged first application still dominates X. This is an I/O improvement, not
a claim that the solver, first reduction, or end-to-end Vakint evaluation became
21 times faster. Whole-process wall times are not compared because old and new
postprocessing differ; exact baseline parsing in the new consumer is untimed.

## Validation and remaining gates

The application suite passed 91/91 tests in a serial run; the candidate subset
contains 18 tests. Candidate and closing CLI suites passed 7/7 each, and the
rebuilt public Python API passed 33/33. Tests include wrong context IDs,
rational-valued guards, exact coefficient/source tampering, affine equations,
separate exceptional conjunctions, nonzero source shifts, negative inactive
terminal indices, non-involutive priorities and provenance text that is not
reparsed. K1/K3 generation, independent certification and application remain
covered. Independent fresh-process tests perturb native symbols and variable
lists and reject conflicting symbol attributes.

Two setup observations are retained rather than hidden: the initial parallel
application suite passed 90/91 with a pre-existing 50 ms presenter scheduling
test failing under load; that unchanged test passed in isolation and in the
full serial run. An initial core-family test invocation used a mistyped license
and aborted; the same test binary passed 5/5 with the correct license. Neither
event demonstrates a native-codec failure. The expanded final core persistence
gate passed 49/49 tests in release mode (0.21 s), including the dense rational
family/Gram/shift cases. The independent fresh-process integration passed in
0.02 s; its ignored child entry is explicitly launched by the parent test.

The subsequent Vakint rollout passes all fifteen named numerical references
and all sixteen expanded-numerator/single- and double-pinch comparisons, with
unchanged tolerances. The public FeynKit/RustRed lane uses an invalid FORM
path; real FORM is available only to the independent FMFT oracle. Three
fixture and three native cold-load/catalog-binding checks also pass. Both
RustRed dependencies are pinned to the pushed `3e621859` revision; no local
source override enters this test result. No catalog, routing, or applier
changes were needed for the binary migration.

The subsequent certified migration is described below. Terminal deduplication
and five-loop work remain separate follow-on tasks.

## Certified artifact V6 follow-up

Certified owners now use the same native envelope and shared Symbolica atom
dictionary as candidates, with an explicit `Certified` kind and V6 proof-record
schema. Base/indexed coefficients, guard polynomials and arbitrary-width affine
matrix integers all use typed references into that dictionary. The old custom
integer-limb/exponent algebra transport has been removed. Registered algorithm
admission, original-source replay, guard/descent checks, root-domain coverage,
homogeneity and nested lower-owner bindings remain unchanged.

Local proof replay resolves each regenerated coefficient to an input ID only
after complete native value and variable-map comparison. Duplicate dictionary
entries are rejected. Final admission independently re-encodes the installed
owner with a fresh interner and compares the complete structural proof and
ordered native coefficient sequence, including its length. This rejects unused
entries and makes ambient Symbolica State bytes irrelevant to mathematical
identity. The public `equivalent_generated_programs` comparison grants no
closure authority itself. Native payload decoding retains the documented
trusted-generator precondition; it is not a hostile-input parser.
The current certified final comparison imports both table snapshots again;
unlike candidate loading, this cold proof boundary does not yet promise a
single native decode. No such work occurs in the reducer hot path.

The broad initial artifact gate passed 519 tests with 13 explicitly ignored;
its sole failure was the expected diagnostic for a forged source being
rejected earlier by native coefficient lookup. That assertion was updated to
the new exact error without weakening rejection.

A separate adversarial comparison test found a genuine implementation defect:
Symbolica intentionally considers equal constants and zero polynomials equal
even when their variable maps differ. Native algebraic equality alone was
therefore insufficient to identify an artifact coefficient. Comparison and
replay lookup now explicitly compare both ordered numerator/denominator maps;
lookup hashing also includes them. Regression tests cover zero, one, negative
constants, empty/reversed/same-width foreign maps and forced hash collisions.
The original 57/58 persistence result is retained in the evidence. Native Atom
interning already preserved these maps, so the saved candidate programs and
converted certified files did not need regeneration or reconversion.

The fresh-process test initially failed before artifact import because its
deliberately reordered symbol-registration setup contained a malformed
namespace. Only that fixture string was corrected to the actual private index
namespace; its portability and reduction assertions are unchanged. This setup
failure is distinct from the genuine map-identity defect above.

After both corrections, the final rebuilt release persistence gate passed
60/60 tests (0.82 s), and the broad artifact gate passed 522/522 with the same
13 intentionally ignored research experiments (67.13 s, 149,336 KiB process
peak RSS). The independent audit restored its source and focused-test signoff.
The final application rerun passed 91 unit tests and 29 focused integration
tests (7 candidate CLI, 7 family-close CLI, 11 closing-artifact API and 4
publication-resource policy). All 34 Python API tests also pass against the
final rebuilt CLI and native extension. Both fresh-process integration gates
also pass: certified K1/K3 portability with reordered native symbol history
(0.04 s) and native coefficient-context portability (0.01 s). Each parent
explicitly launches its otherwise-ignored child entry points in new processes.

## Vakint public-backend follow-up

The existing nine-input scalar benchmark passed all 54 numerical comparisons
(108 backend calls, five paired repeats after the initial pair), relative
tolerance `1e-20` with zero uncertainty allowance. It used the same release
configuration, CPU affinity 88–93 and one scalar caller, with nested pools
capped at one. These are separate shared-host diagnostic observations, not a
controlled paired statistical speedup claim.

| Quantity | Previous text programs | Native programs |
| --- | ---: | ---: |
| Complete nine-input process wall time | 251.79 s | 167.33 s |
| Whole-process peak RSS | 14,686,292 KiB | 2,863,816 KiB |
| First H cubed-propagator call | 35.353 s | 15.962 s |
| First FG cubed-propagator call | 11.450 s | 1.643 s |
| First BMW cubed-propagator call | 20.907 s | 6.288 s |
| First X cubed-propagator call | 127.140 s | 82.864 s |

The native process used 161.29 s user CPU and 4.85 s system CPU. First-parent
calls include lazy loading and the first uncached scalar reduction; they are
not loader-only timings. Repeated RustRed medians range from 15.136 to
150.665 ms, versus 132.988 to 4,662.828 ms for FMFT on those same inputs.
Previously unseen targets remain a separate cost: the initial FG/X expanded-D7
calls took 1.610/2.934 s even after their parent program had loaded. Neither
cache-repeat timings nor the I/O improvement eliminate this application cost.

The numerical reference suite itself took 32.45 s wall and peaked at
1,989,496 KiB; the sixteen-pinch suite took 104.98 s and peaked at
2,696,884 KiB. These include oracle work and must not be presented as standalone
RustRed generation or reduction timings. Exact test invocations and the
nine-row public timing table live with Vakint's shipped four-loop data README;
raw local evidence is under `TMP/gamma-native-migration-20260919/`.

## Certified native-format conversion

A separate workspace-only converter migrates the existing shipped K1/K3/K6
certified files, without running rule discovery. It retains the frozen V5
decoder's complete exact replay and old-format comparison, installs the same
semantic owner under the current schema, then uses the new V6 encoder. Shipped
input bytes are not edited. This tool is not a runtime compatibility layer.

| Shipped artifact | V5 bytes | Native V6 bytes | Unique coefficients | Rule cells | Masters |
| --- | ---: | ---: | ---: | ---: | ---: |
| K1 | 2,736 | 2,790 | 9 | 0 (one direct rule) | 1 |
| K3 | 50,525 | 36,692 | 56 | 5 | 2 |
| K6 | 8,916,759 | 3,725,739 | 1,635 | 5,639 | 38 |

The K6 file is 58.2% smaller; K1 grows by 54 bytes from native framing overhead.
The shipped K6 census is distinct from the example's 5,640-cell producer
snapshot. Neither is being regenerated or silently substituted for the other.
Conversion compares complete structural/native payloads, sources, family and
ordering, nested lower owners and exact target reductions. Fresh-process
verification independently replays and reduces each native output.
Canaries cover the all-one integral, all-zero integral, first-line dot and
first-line pinch (three distinct targets at K1). The K6 dotted result has the
same 30 exact terms before and after migration. Ordinary source counts remain
1/4/9; K3 retains its nested K1 owner.

A release diagnostic on CPU affinity 50–51 measured fresh K6 decode/replay at
0.815396 s, 0.91 s whole-process wall and 239,640 KiB peak RSS. This is **not
K6 generation time**, nor a controlled comparison against V5. The same-process
conversion's old/new decode observations (0.907787/0.820725 s) have different
initialization and state histories and are not a cold speedup benchmark.
All RustRed migration gates and the atomic Vakint pin/asset update are complete.
The separate K1/K3 fresh decode-and-replay phases took 0.000697/0.003912 s.

The final rebuilt Python extension also cold-loaded each migrated file in its
own process and reduced the first-line-dotted target. All exact typed-master
coefficients and homogeneity exponents match the retained CLI baseline: one,
one and thirty output terms for K1, K3 and K6 respectively. This is an exact
regression check, not a new generation or performance measurement.
The final CLI independently cold-loaded and reduced those same three files;
its complete inspection and reduction reports match the retained baseline.

### Final atomic Vakint rollout

GammaLoop commit `39992f757fce31acf08f075dce8fea13d7eec8d2` is pushed to
`vakint_rustred`. Both RustRed dependencies pin the published
`d6718733bee4d20f4554e9d694b288ddaae60475` revision. All three native certified
files match the audited converter hashes; the four-loop candidate programs,
master catalogs, routing and scalar applier are unchanged. Old certified files
remain recoverable in Git and the local evidence directory.

The unchanged 83-case selection passes in all nine test binaries, including
the explicit offline MATAD comparison for all 38 K6 terminals. Its cumulative
process wall time is 32.94 s. The independent four-loop gate passes all 15
named numerical references and all 16 expanded-numerator/pinch cases, without
a family filter and with invalid FORM paths in both native stages:

| Fresh pinned public gate | Named cases passed | Whole-process wall | User / system CPU | Peak RSS |
| --- | ---: | ---: | ---: | ---: |
| Four-loop numerical references | 15 | 34.42 s | 31.27 / 2.87 s | 1,988,896 KiB |
| Four-loop numerator/pinch comparisons | 16 | 104.55 s | 99.62 / 4.00 s | 2,696,252 KiB |

These are correctness timings with oracle work included, not standalone
RustRed performance. The two four-loop processes ran sequentially on CPUs
80–85 with nested pools capped at one; the through-three-loop gate ran on
CPUs 88–93. The earlier nine-input performance snapshot above was not rerun
for this format-only certified migration.

Additional focused gates pass: 26 native library tests, all 16 K6 pipeline
tests and three four-loop fixture checks. They overlap the 83-case selection.
The library's one normally ignored offline master check was explicitly run
within the 83-case gate; four ignored public diagnostic/oracle/benchmark entry
points retain their existing flags, with both required numerical entries run
explicitly. Numerical tolerances and assertions are unchanged apart from
format-specific schema and native semantic-identity expectations.

`cargo hakari generate` reports no changes, `cargo hakari verify` passes and
`just ci-update` passes without generated-file drift. The focused release build
passes, formatting and Git whitespace checks are clean, and independent source
and factual audits approved the atomic commit. This is not a whole-GammaLoop
workspace CI claim. This establishes native IBP-program persistence. A broader
audit subsequently identified the small text terminal-value catalogs as the
remaining internal-algebra persistence migration; see the updated I/O plan.
Terminal normalization remains a distinct task and is not part of these timings.

## Reproducibility evidence

The unchanged inputs are in `TMP/vakint-four-loop-assets-20260919/`. The
disposable converter, four native outputs, complete comparisons, process timing
records and matched level-6 compression are in
`TMP/candidate-native-migrate.DnjH0n/`, with `MIGRATION_REPORT.md` collecting
counts and file identities. Conversion times include legacy parsing and exact
comparison and are not production load benchmarks.

Independent old/native consumer sources, exact terminal-output logs,
phase counters, GNU time records and level-9 files are under
`TMP/four-loop-native-benchmark/`. Implementation/audit design is in
[`native_binary_io_plan.md`](native_binary_io_plan.md). Broader previous loader
profiling is in
[`four_loop_cold_profile_2026-09-19.md`](four_loop_cold_profile_2026-09-19.md).
The certified converter, unmodified-input checks, native outputs, exact-owner
comparisons and separate verifier logs are in
`TMP/certified-native-migrate.rRNAR9/`. The scratch snapshot's explicit legacy
wire-schema adapter is not included in the production library.
Final rebuilt gates are recorded in
`TMP/native-certified-{persistence,artifact,contexts}-verified-final.log`,
`TMP/native-certified-app-final.log`, and
`TMP/native-certified-python-api-verified-final.log`.
`TMP/native-certified-cli-canary-comparison.log` and
`TMP/native-certified-python-canary-comparison.log` record the complete
post-fix canary comparisons. Formatting and Git whitespace checks pass.
The final pinned Vakint build manifest, executable hashes, unchanged 83-case
selection, all public-gate logs and resource records are under
`TMP/gamma-certified-native-migration.XbZB8J/`. This directory also retains
the replaced `.rr` inputs; the shipped `.rrbin` files were copied from the
audited converter, not regenerated by the acceptance run.
