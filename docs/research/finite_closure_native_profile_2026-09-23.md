# Profiling the saved-rule closure visitor

## Measured bottleneck

A completed, bounded A11/R2/D9 traversal was sampled with Linux `perf` after
the release build and the six scheduler controls had finished. This workload
uses four saved owners, 86 routes and four starting regions representing
45,342 integral keys; it does not regenerate IBPs or represent all 67 owners.
Every descendant remained required. The native receipt discharges all 27,806
native inspections (including 1,492 partial initial-overlap inspections), plus
9,630 delegations, with zero frontiers, errors or pending obligations. Scheduled
descendant rank
bounds reach three; that is not proof of concrete rank-three reachability.

The frozen executable is `58002e5136efc622f8d3a33a6e979550da79e108cfbf8bfe704e741e28c2db7d`.
The diagnostic used Ordered execution with 26 workers on CPUs 0–25, not the
50-worker scheduler comparison. Its instrumented traversal took 11.681 s;
preparation took 2.693 s. Profiling overhead and the different worker budget
make this unsuitable as a before/after timing control.

| Exclusive sampled user CPU | Share of whole recorded process tree |
|---|---:|
| Symbolica integer-polynomial `replace` | 32.81% |
| Symbolica rational-polynomial context `Arc::drop_slow` | 11.34% |
| Symbolica `append_monomial_back` | 5.63% |
| RustRed `validate_polynomial_on_map` | 4.85% |
| Symbolica integer-polynomial `new` | 4.16% |
| RustRed power-domain `project` | 3.35% |

Inspector threads account for 91.63% of sampled user CPU, admission helpers
2.79%, and the main thread 5.51%. These are CPU shares, not wall-time shares or
proof that changing a function will save that percentage of elapsed time.
The trace contains 9,464 samples, no lost samples and no unresolved leaf symbols.
Native worker stacks did **not** unwind, so caller/inclusive attribution is not
available. Main-thread samples also include initialization and output. In
particular, the profile does not prove that all replacements originate in the
fixed-index specialization function.

The profile used spawned descendants only, user-space `cpu-clock` at 99 Hz,
DWARF stack capture, local cache paths and no network symbol download. Native
supervision retained its 24/32-GB soft/hard tree-memory policy; the separately
sampled recorder peaked at 13.54 MB. GNU time reported 589,216 KiB maximum RSS,
which is not a summed tree peak. No root build or timing control overlapped.

## Small native-API experiment

The public API audit checked declarations, implementations and existing native
tests/callers in the pinned, locally patched Symbolica 3.0 source. Public
`replace` already dispatches to `replace_last` when all higher variables are
absent. Otherwise it reconstructs and sorts the polynomial. An absent-variable
replacement can also incur that reconstruction unnecessarily.

The proposed generic change retains Symbolica's algebra and changes only
independent constant-substitution order:

1. Substitute all zero-valued fixed coordinates first.
2. Substitute the remaining fixed coordinates in descending original
   variable-map order.
3. Skip a substitution when native `degree` proves the variable absent.

Calling `replace`, rather than unconditionally calling `replace_last`, preserves
arbitrary higher **unfixed** variables. No variable map is compacted. No new CAS,
cache, topology-specific rule or artifact schema is introduced.

Zero-first is required for the prospective resource bound: specializing
`x*y^65535` at `x=0, y=i64::MAX` must not first construct the enormous power of
`y`. The preflight already recognizes terms annihilated by any zero assignment.
The old ascending order also has this hazard when the zero coordinate occurs
later. Zero-first removes such terms before any growing nonzero power.
This change concerns the shared **fixed-index partial-specialization** path;
the separate full-assignment specialization path is not changed by this slice.

Preserve all original preflight checks, normalization, context authentication,
and the pre-cancellation denominator guard. A zero numerator must not conceal
a zero denominator. Affine-chart construction stays in its existing order;
only the subsequent independent scalar constants commute.

Other native APIs were considered, not overlooked. `replace_except` retains
only one variable and is unsuitable for arbitrary partial specialization.
`evaluate_with_coeff_map` with a polynomial coefficient ring is a genuine
simultaneous alternative, but is a larger unmeasured change. Reusable native
last-variable workspaces are private; RustRed does not copy them.

The implementation passes independent source review and **2,798 core release
tests, zero failures, 32 existing ignored**. The 72-test indexed-algebra subset
overlaps that total. Six new tests include 192 deterministic polynomial/subset/
assignment comparisons against the old ascending native Symbolica path, zero
annihilation before huge powers, retained poles, `i64::MIN` and unchanged
prospective refusals for absent variables. No existing assertion is weakened.

The release CLI build also passes. Gate evidence:
`TMP/fixed-substitution-release.whEAZx/`; independent source review:
`TMP/fixed-substitution-independent-audit.UdEhIf/REPORT.md`.

## Completed matched substitution comparison

Six sequential Ordered A11 runs alternate baseline/new, new/baseline,
baseline/new. Baseline is the completed-slot milestone
`be31322c2908deca0543bc7a7cd6e44de48e0347078dcbe8ee30b4ae9d731eea`;
the new immutable CLI is
`e5a95023e4334accac893bf7a28086546883e92c461181e7b7f29267d4a52dec`.
All use the same saved rules, starting input, ordering, 50-worker budget,
CPUs 0–49, observer on CPU 50, H256 and resource allowances. Descendants remain
uncut. There is no compilation, profiling, regeneration or elapsed deadline
inside a control, and no competing root build/run. Other shared-host work remains
possible. Each process loads its owners afresh; OS caches are not forcibly cold.

| Pair | Baseline traversal (s) | New traversal (s) | Baseline process CPU (s) | New process CPU (s) |
|---|---:|---:|---:|---:|
| 1 | 11.647093 | 5.938401 | 147.62 | 51.06 |
| 2 | 11.770175 | 5.875382 | 147.14 | 50.82 |
| 3 | 11.590157 | 6.018743 | 141.51 | 51.62 |

Median traversal falls **49.01%, from 11.647093 s to 5.938401 s (1.96×)**.
Median whole-process CPU falls from 147.14 s to 51.06 s, about 65.3%. Median
peak process RSS rises from 572,380 to 595,792 KiB (about 4.1%); this is not a
measured memory reduction. Corrected sampled busy-core medians fall from 12.41
to 8.35: less CPU is spent doing the same work, rather than more cores becoming
occupied. Whole-process CPU/RSS include setup/finalization; traversal has the
same post-preparation timing boundary as the scheduler controls.

All six runs pass the raw scope/responsibility and structural-equivalence checks.
They agree exactly on the non-timing structural report and counters: 27,806
native inspections, 695,918 events, 3,227,881 counted native operations, 9,630 discharged delegations
and 1,492 discharged partial inspections (included in native totals). No
frontiers, unresolved responsibilities or descendant clipping are introduced.
The matching structural digest is
`b4b2568a8555fe0e43b9e177ee4fd9ee3ca4dce5b145e327c17c8bbc972f7947`.
This establishes unchanged results on the complete scoped pilot, not regenerated
source provenance or exhaustive correctness for arbitrary unseen inputs.

These are three descriptive repeats on a shared host, not a confidence interval.
In particular, this block's baseline is slower than the earlier scheduler block;
compare binaries **within this alternating block**, not across separate host
conditions. Do not multiply this gain by the owner-local scheduler gain: this
experiment uses Ordered exclusively. The larger-input and post-change profile
gates below are now complete; neither establishes a broad-run ETA or attributes
all savings to one caller.

Evidence: `TMP/indexed-substitution-a11-compare.5Jhj9C/matrix/`. The six-run
steering exits zero after exact comparison checks. Independent post-run raw
measurement review passes, including independently recomputed CPU windows and
resource/obligation checks:
`TMP/fixed-substitution-measurement-audit.5MPRu0/REPORT.md`.

## Larger A12 control: the improvement persists

The same two frozen executables completed three alternating Ordered A12/R3/D9
pairs, with unchanged four owners, 86 routes, 50-worker allocation, affinity,
H256, monitoring and resource allowances. Four entry regions represent 357,192
starting tuples. Descendants remain uncut, with scheduled rank bounds reaching
four. Each process reloads the saved owners; there is no IBP regeneration,
compilation, profiling or root-controlled competing native workload during timing.

| Pair | Baseline traversal (s) | New traversal (s) | Baseline process CPU (s) | New process CPU (s) |
|---|---:|---:|---:|---:|
| 1 | 39.901876 | 19.681471 | 441.51 | 163.52 |
| 2 | 38.915977 | 20.511271 | 430.38 | 168.26 |
| 3 | 38.400460 | 20.748213 | 430.79 | 164.58 |

Median traversal is **38.915977 → 20.511271 s: 47.29% lower, or 1.90× faster**.
Median process CPU is 430.79 → 164.58 s, 61.80% lower. Median peak process RSS
is 1,409,884 → 1,389,400 KiB, 1.45% lower; this is not a summed tree peak.
Corrected sampled busy-core medians fall from 11.00 to 8.08. Again, this saves
work per inspection rather than establishing better fifty-core occupancy.

Every run completes with exactly 62,562 native inspections, 2,507,532 events,
12,313,872 counted native operations, 33,199 discharged delegations and 2,737
discharged partial initial-overlap inspections (included in native counts).
The identical structural digest is
`1920836d9b4f3f9af68a1ecf9e8b8f63b17564787b211ff73b95d26763bc3b1b`.
Scope, anchors, residual responsibilities, drained pools and resource checks
pass; there are no frontiers, pending obligations or errors. An independent raw
audit reproduced the checks and medians before subsequent steering edits.
Three repeats on a shared host remain descriptive, not a confidence interval
or a prediction for all 67 owners.

Evidence: `TMP/indexed-substitution-a12-compare.MxAoFr/matrix/`; independent
review is appended to the existing measurement audit cited above.

## Post-change CPU profile: the dominant cost has moved

A separate completed diagnostic used the new frozen executable on the same
A11 workload, Ordered W26, with the same perf configuration as the original
profile. All original native counters and responsibilities match. Native
traversal took 5.649 s and the whole recorded command 9.769 s; these instrumented
figures are **not** another matched timing comparison.

| Exclusive sampled user CPU | Before | After |
|---|---:|---:|
| Symbolica integer-polynomial `replace` | 32.81% | 1.79% |
| RustRed power-domain `project` | 3.35% | 10.39% |
| RustRed `validate_polynomial_on_map` | 4.85% | 3.45% |

The larger percentage for `project` does not show a regression: its absolute
sampled CPU weight is approximately 3.20 s before and 3.22 s after, while the
total sampled CPU denominator shrinks. Allocation, hashing and polynomial
degree/preflight checks now also matter. This suggests measuring reusable domain
geometry and structural rule data before adding a broad proof cache.

The new recording has 3,070 samples, zero reported loss and zero unresolved leaf
symbols. Inspectors account for 75.99% of sampled user CPU, admission helpers
6.78%, and the main thread 17.10%, including setup and output. Worker call stacks
still do not unwind: caller/inclusive attribution remains unavailable. These
shares cannot be read as parallel critical-path time. An opt-in inspector/helper
partition experiment is therefore justified, but not presumed faster.

Evidence: `TMP/post-substitution-profile.C2n5bq/run/analysis.json` and adjacent
raw perf/receipt files. Independent analysis replay passes, including PID/receipt
binding, loss accounting, input immutability and drained native work. A syntax
error in the first postprocessing script was corrected and analysis rerun on
the same recording; no native rerun or sample selection was involved.

## Explicit worker partition: completed same-input comparison

The generic Rust/CLI/Python option `inspection_workers` / `--inspection-workers`
permits a different inspector/helper split without changing total workers or
publication policy. Existing defaults stay unchanged. Release validation passes
539 application/CLI unit tests (zero failures, one existing ignored), 30 Python
steering tests and the executable build. The first test compilation needed one
explicit `usize` annotation in a new test; no production assertion was weakened.
Independent implementation review passes.

The fresh frozen CLI is
`7d493839995ec94049970a7a68d2c079d0b81975f06729ed39eaf4ce2468231a`.
An initial default-versus-explicit-I25 gate gives identical structural results
and counters, at 20.223 and 20.273 s. Those two timings are excluded from the
following medians. Nine further A12 controls rotate default/I40/I48,
I40/I48/default, I48/default/I40. All share the same saved inputs, ordering,
H256, W50, CPUs 0–49, observer CPU50, serial inner pools and resource policy as
the earlier A12 controls. No root build or other native run overlaps the matrix.

| Inspectors / admission helpers / coordinator | Median traversal (s) | Median process CPU (s) | Sampled busy cores | Median peak process RSS (KiB) |
|---|---:|---:|---:|---:|
| 25 / 24 / 1, unchanged default | 20.239343 | 166.33 | 8.10 | 1,399,808 |
| 40 / 9 / 1 | 20.160298 | 136.14 | 6.59 | 1,422,920 |
| 48 / 1 / 1 | 23.835257 | 127.77 | 5.24 | 1,355,276 |

Forty inspectors offer **no established wall-time improvement**: the median
difference is only 0.39%, with overlapping repeats on a shared host. They use
18.15% less process CPU. Forty-eight inspectors are 17.77% slower in median,
despite using less CPU. More configured inspectors do not imply more useful
concurrency. Keep the default; this does not justify a fifty-core scaling claim
or restarting the full envelope unchanged.

Independent heartbeat-window analysis finds median inspection activity of
4.52 / 5.13 / 4.40 cores and helper activity of 2.94 / 0.84 / 0.30 cores for
default / I40 / I48. These role medians need not add to the median total. They
show that the extra inspector reservation is not being usefully filled, but
do not identify a particular spin, lock, cache or critical-path mechanism.

All eleven controls complete with identical structural results and counters:
62,562 native inspections, 2,507,532 events, 12,313,872 native operations,
33,199 discharged delegations and 2,737 discharged partial inspections. All
descendants remain required, with scheduled rank bounds reaching four. There
are no errors, frontiers, unresolved obligations or retained pool work.
Evidence: `TMP/worker-split-pilot.vkl0F5/matrix/`; release gate:
`TMP/worker-census-release.NZeG5m/`. Independent raw result review passes and is recorded
at `TMP/worker-partition-independent-audit.2rJLon/`.

## Relation to the radical redesign

The [independent architecture review](finite_closure_architecture_review_2026-09-23.md)
targets the *amount* of overlapping closure work. This profile identifies a
concrete cost inside each native inspection. Both matter: speeding one visit
does not prevent an expanding reachability history, and a new parallel scheduler
can increase repeated work.

A separate structural analysis found no duplicate complete native query keys
in the old A11 stream. Repeated fixed-coordinate layouts therefore do not yet
establish a safe proof-cache hit rate. Prepared rule-transfer plans, exact union
differences and a finite closed cover remain separate experiments requiring
complete, cold comparisons—not inferred gains from these CPU percentages.

## Local reproducibility evidence

Ignored, workspace-local evidence is retained in:

- `TMP/native-transfer-profile.qTo5Mq/`: driver, frozen input hashes, raw perf
  recording, bounded parser, `run/analysis.json`, full report and public-API audit;
- `TMP/compiled-transfer-profile.QNqGqU/`: read-only structural counts and parser;
- `TMP/owner-retention-a11-steering.d7SNxy/matrix/`: the separate six completed
  scheduler timing controls and unchanged native inputs.

These results establish scoped operational coverage relative to saved rules.
They are not full five-loop closure, source-identity certification, master
minimality, fifty-core saturation or a full-campaign completion estimate.
