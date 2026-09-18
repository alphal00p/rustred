# Vakint four-loop candidate/catalogue audit — 2026-09-18

This note is an independent, read-only audit of the completed release run
`TMP/vakint-4l-catalog-20260918b/all-parents-w6.log`, the four offline
catalogues in the same directory, and the already-finished native
RustRed/SpIRed canaries.  It is deliberately not a family-closure or
certification claim.  The candidate reducer remains an experimental bounded
finite-target lane.

## Completed all-parent release run

The test is Vakint's ignored
`candidate_all_four_loop_parents_match_fmft`.  It contains four independently
matched four-loop parent families (`H`, `FG`, `BMW`, `X`) and three probes per
family (parent, dotted, pinch), hence twelve finite targets.  The run completed
with:

```
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;
finished in 895.67s
```

The executable was the release-profile binary
`FOR_REFERENCE_ONLY_DO_NOT_PUSH/gammaloop/target/release/deps/experimental_rustred_4l_tests-d754d678640c9c42`
(SHA-256 `b0a50d7e64b5c60cb83f7a6106cc4049b452263a35e9ba596857b963da01a56`).
The test's six-worker setup is identified by the `all-parents-w6` run label and
the worker environment used by the corresponding catalog-only rerun; the
finished process is no longer available for an environment dump, so the
completed log alone cannot independently reconstruct its shell command.
There is no separate process/RSS record for this completed run, and its
895.67-second test time must not be presented as a solver-only benchmark.

The recorded candidate searches and finite-target outcomes are:

| family | candidate search | fixed residuals | dotted applications | reached dotted terminals | parent/pinch | parity probes |
|---|---:|---:|---:|---:|---|---:|
| H | 131.40127209 s | 386 | 26,956 | 248 | 0 / 0 | 3 / 3 |
| FG | 104.61306281 s | 145 | 3,362 | 78 | 0 / 0 | 3 / 3 |
| BMW | 129.532918904 s | 179 | 16,113 | 179 | 0 / 0 | 3 / 3 |
| X | 130.986692097 s | 445 | 82,637 | 437 | 0 / 0 | 3 / 3 |

For every row the log contains
`finite-target numerical parity PASS: <family>/<probe>` for all three
probes.  It also contains an invalid-FORM scalar-tail check for every family
(26,956, 3,362, 16,113, and 82,637 applications respectively), so these
checks do not require a runnable FORM executable.  The FMFT/FORM executable in
the non-catalog run is an oracle used to construct/check finite terminal
values, not a RustRed rule source.  None of these twelve probes proves
whole-family closure.

## Offline terminal catalogues

The catalogues are the exact Symbolica textual records currently staged for
the candidate lane.  They contain rational/symbolic expressions in the Vakint
master atoms, rather than decimal approximations; a scan found no decimal
floating-point literals in any terminal value.  Each uses schema 1 and ten
integer indices.  Terminal keys are unique and have the declared arity:

| file | terminal records | bytes | SHA-256 | index count | malformed/duplicate keys |
|---|---:|---:|---|---:|---:|
| `h.rrcat` | 249 | 18,349 | `a93391609db06193e872f6884ab2028769e23cc11727d119003d5285f5641d85` | 10 | 0 / 0 |
| `fg.rrcat` | 78 | 7,129 | `d7b89dcf70b623347f601a5f54a5124462483ced614ee3802617629f9defc2b9` | 10 | 0 / 0 |
| `bmw.rrcat` | 179 | 13,496 | `dd0d6e41aab8d2a359ba34996381d118a197b9812af6dc34a1edd0437562509f` | 10 | 0 / 0 |
| `x.rrcat` | 437 | 32,861 | `e3a14f3223b3b3edbbacbf8fb7abc851ff197423aa6d1834a841b7ee5e8d85d8` | 10 | 0 / 0 |

The four `family_fingerprint` lines are intentionally different because the
four graph families have different denominator incidence; each catalogue must
be decoded against its own family fingerprint.  A malformed or wrong-family
catalogue is rejected at the Vakint load boundary.  The finished all-parent
run loaded the H catalogue and generated/checks the other finite terminal
values through its explicit FMFT oracle.  The separate catalog-only rerun
(`all-parents-catalog-only-w6.log`) is the relevant FORM-free cold-load check:
its captured log loaded and passed H, FG, and BMW.  The process subsequently
disappeared after the BMW line without writing an X section or a Rust test
result/exit record, so it is an interrupted/incomplete run rather than a
four-family pass.

The in-progress catalog-only process was independently observed as the same
release binary under `time -p`, with:

```
VAKINT_4L_CANDIDATE_WORKERS=6
VAKINT_4L_CANDIDATE_CATALOG_DIR=TMP/vakint-4l-catalog-20260918b
VAKINT_4L_CANDIDATE_CATALOG_ONLY=1
VAKINT_4L_CANDIDATE_ORACLE_FORM_PATH=/definitely/invalid/form
```

Its environment confirmed the intended no-FORM path; because no X section or
completion record was captured, any replacement run must be recorded
separately from the completed oracle-backed run above.

## Native RustRed/SpIRed coefficient equality

`TMP/rustred-four-loop-cpp-matched.VRhy3D/native-canaries-final-checked.json`
contains twelve completed exact coefficient comparisons at integer targets:
four H, three FG, and five BMW (including parent/pinch controls and
factorized-dot controls).  The C++ and Rust bases are not byte-compatible, so
the test transports both sides to a common terminal basis and compares exact
Symbolica coefficients.  Every comparison has `exact_equal=true` and
`nonzero_differences=0`; the transported native display files are also byte
identical.  Representative recurrence counts are:

| family | Rust applications | Rust terms | C++ terms | C++-only keys | process wall | peak RSS |
|---|---:|---:|---:|---:|---:|---:|
| H dotted | 26,956 | 248 | 285 | 40 | 288.12 s | 6,985,788 KiB |
| FG dotted | 3,362 | 78 | 99 | 21 | 84.43 s | 3,256,380 KiB |
| BMW power-three | 16,113 | 179 | 217 | 42 | 249.85 s | 4,988,988 KiB |

These are correctness canaries, not performance comparisons (the protocol
explicitly labels them debug-linked diagnostics).  There is currently no
corresponding exact C++↔Rust numerical canary for X.  Therefore the X family
has finite-target FMFT parity in the Vakint log and a completed C++ structural
workload below, but not yet the native cross-implementation coefficient
equality evidence available for H/FG/BMW.

## C++ X status and remaining holes

The completed uncapped six-worker SpIRed X workload is
`cpp-x-w6-uncapped-resume2-20260917.output`:

```
status=uncertified-generated   workers=6   sectors=328
rules=19803                    finite_residuals=597
prepare_us=3310                solve_us=27500179946
output_us=10689468             total_us=27510872725
wall_seconds=27512.12          peak_rss_kib=8192404
exit_code=0
```

It has 328 non-empty `.rules` files and 329 non-empty `.dat` files.  This is
an uncapped generated census, not a mathematical equivalence proof.  The
one-worker X process (PID 3831904, output label
`cpp-x-w1-uncapped-resume3-20260917`) remains live: at audit time it had
11h19m elapsed, about 90.7% CPU, 4.81 GiB resident memory, no output files,
and no `.time`/status completion record.  It must not be reported as finished,
timed out, or equivalent to RustRed.

The remaining concrete validation holes are therefore:

1. finish and archive the catalog-only X probe, then preserve its fresh
   no-FORM output and resource metadata;
2. add a bounded exact X native canary (or explicitly document why its C++
   terminal basis cannot yet be transported) rather than inferring equality
   from rule/file counts;
3. keep the four finite catalogues clearly marked as experimental bounded
   value caches, not certified whole-family artifacts.

No process was started, stopped, or modified for this audit, and the live
catalog-only and C++ X processes were inspected read-only.
