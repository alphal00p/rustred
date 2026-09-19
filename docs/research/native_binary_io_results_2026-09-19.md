# Native candidate I/O: implementation and measured migration

## Scope and result

The four saved four-loop candidate programs were converted without generating
any new IBPs. Exhaustive same-process and separate fresh-process comparisons
preserved all 59,636 rules, 1,155 declared residual keys and 1,579,493 coefficient
uses. The new programs contain native Symbolica rational-polynomial atoms,
their shared state context, native family geometry and structural rule records.

This completes the candidate-format implementation and its saved-data migration,
not the whole uniform-I/O roadmap. Certified artifacts still use their existing
proof codec; migrating that codec to the common envelope must retain independent
source replay and closure admission. Transport equivalence and successful
concrete reductions do not certify unrestricted family closure.

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

Pending work at this checkpoint: install the native programs in Vakint and
repeat its public numerical/pinch acceptance
gates, and migrate certified proof-bearing artifacts into the common native
container without weakening replay. Terminal deduplication and five-loop work
remain separate follow-on tasks.

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
