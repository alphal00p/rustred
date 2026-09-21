# Rank-scoped candidate generation

The optional `max_numerator_rank = R` means
`sum_i max(-n_i, 0) <= R` for **starting integral indices** in the supplied
family presentation. It counts scalar-product numerator degree, not tensor
momentum rank. Positive denominator powers are unbounded. This differs from
the optional certification scope `max_total_excess_degree`, which also counts
raised denominator powers.

The active five-loop goal is all-family coverage at least through R=10,
ideally R=20, allowing a finite nonminimal terminal set. Only afterward come
terminal minimization, published numerical master values and Vakint five-loop
integration. Small examples and isolated exceptional branches do not satisfy
the complete census milestone.

## Public interfaces

The existing Rust application request adds one optional field:

```rust,ignore
let mut request = FamilyCandidatesRequest::new(family_input);
request.max_numerator_rank = Some(10);
let generated = family_candidates(request)?;
```

With the CLI, add `--max-numerator-rank 10` to `family-candidates`, alongside
the existing family input and root-sector options. For example, this selected
six-line five-loop pressure case is **not** the entire five-loop census:

```sh
rustred family-candidates \
  --input examples/input/tide_five_loop.toml --input-format toml \
  --nonpositive-indices 3,4,5,6,7,8,9,10,14 \
  --max-numerator-rank 10 --n-cores 1 --exact-backend sparse \
  --numerical-depth 0 \
  --checkpoint-dir TMP/tide-rank10 --progress \
  --output TMP/tide-rank10.rrbin
```

Python's existing `rustred.family_candidates(...)` accepts the same keyword
`max_numerator_rank=10`. Omit it for the unchanged unrestricted path. Rank zero
is valid and still permits arbitrary raised denominator powers. `numerical_depth`
is a separate search radius around fixed cases, not the numerator rank.
The depth-zero example still checks the initial fixed-case IBP sources. It
keeps any remaining fully fixed cases as explicit residuals instead of
spending deeper search effort on shrinking their number; symbolic case
search remains unbounded. This is appropriate for the initial coverage-first
milestone, and does not establish independence of those residuals.

### Deliberately retaining finite leaves

The follow-up implementation adds `finite_case_policy="retain-rank-finite"`
(CLI: `--finite-case-policy retain-rank-finite`, Rust:
`FiniteCasePolicy::RetainRankFinite`). It requires an explicit rank and differs
from `numerical_depth=0`: whenever every active original index is fixed, it
enumerates the remaining inactive-coordinate total-degree simplex, intersects
each point with the **whole** affine case, and keeps all admitted points as
explicit nonminimal terminals. Fully numerical leaves likewise bypass search.
Any unfixed positive direction continues through ordinary parametric solving.
The follow-up release gate passes 2,373 core tests (32 existing ignored),
152 application tests, 15 CLI tests and 43 Python API tests. Eight additional
K1/K3 R=10/R=20 serial/parallel retention smokes pass. A malformed delta-sector
test fixture required its missing delta flag; assertions were unchanged and
the failed baseline receipt is preserved separately.

The default remains `finite_case_policy="search"`. Per-sector limits
`finite_max_visited_points` and `finite_max_retained_terminals` default to
1,000,000 each (CLI names use hyphens; Rust stores them in `finite_case_limits`).
They limit work/storage, not the mathematical domain. Exhaustion returns an
incomplete error, not a successful prefix. The generation policy and checkpoint
identity bind retention and its limits; old search checkpoints cannot silently
be resumed as retention campaigns. This option may save expensive numerator
recurrences but lose useful above-R coverage from those recurrences. Internal
successors remain unclipped and must still be resolved: finite local retention
is not a successor-closure proof.

The first selected six-line five-loop pilot through R=10 saved six of its
seven nonzero sectors, including the parent, in **50.93 s wall / 59.35 s CPU**,
peaking at **261,212 KiB RSS**. It saved **1,044 rules and 1,024,045 finite
terminals**. The seventh sector, ordinal4 / mask`111000000001010`, failed
at 1,000,001 cumulative visited points against the declared 1,000,000 limit.
The process correctly exited8 without assembling a final candidate bundle.
Its six saved shards are preserved, not relabelled for another policy budget.
This was four workers, sparse-factorized arithmetic and nested Rayon1; the
earlier one-hour pilot used six workers and reconstruction. The difference
is not a controlled speedup measurement. Evidence:
`TMP/tide-r10-finite-four-workers.f7ADgX/` and
`TMP/finite-retention-gate.qs4pQb/RESULTS.md`.

A fresh retry, with the public Rust request's visited-point limit raised to
10,000,000 and larger explicit native transport budgets, **writes all seven
candidate sectors** in **38.84 s wall / 46.55 s CPU**, at **373,256 KiB peak
RSS**. It produces1,299 rules,1,208,801 nonminimal terminals and a41,971,427-byte
bundle. Its reported preparation/generation/encoding times are9.337/27.183/1.792s;
no checkpoint was reused. The small external driver and native libraries are
optimized; compilation is outside this process measurement. This demonstrates
completion of local candidate generation for this selected family, not all
five-loop topologies or recursive rank10 closure. Cold loading and actual
successor checks are required next. Evidence:
`TMP/tide-r10-finite-larger-budget.9QDI7r/`.

### Four-parent coverage campaign

The expanded user budget permits up to100 compute cores and500GB aggregate
RAM. Four independent release campaigns use20 sector workers each, with
disjoint affinities, native inner pools limited to1, persistent checkpoints,
80GiB per-process address-space limits and an initial two-hour reassessment
deadline. A separate16-core release gate and4-core diagnostic allocation fit
inside the100-core ceiling. The monitor counts their actual process trees and
soft-stops owned solvers at450GB aggregate sampled RSS, leaving headroom.

The independent 07:48 UTC snapshot contains **779 distinct saved labelled
sectors out of8,246**, after removing30 duplicate saved occurrences between
the overlapping parent downsets. All four12-line parents have saved shards.
The779 masks route to52 of67 published classes. These are generation/storage
counts, not completed recursive reductions or certified family coverage.
Many pending masks have not started, so they must not be labelled hard cases.
See `TMP/rank10-inventory-audit.9t07iu/AUDIT.md` and the ongoing batch in
`TMP/tide-r10-four-parent-batch.yGOAw6/`.

Two distinct failures have appeared. A nonlinear exceptional equality
`8-3*n6-6*n2+2*n2*n6=0` remains unsupported. Its equivalent integer equation
`(2*n2-3)*(n6-3)=1` admits only the pairs `(2,4)` and `(1,2)`, subject to the
full original case; this is a missing exact finite-case refinement, not proof
of an infinite residual direction. Separately, some native exports exceed
the default128MiB total coefficient-table budget. The Rust request already
exposes `bundle_limits.max_total_coefficient_bytes`; increasing checkpoint
disk space or process RAM does not increase that independent limit.

A preparation-only control using the same input, an empty root and the frozen
CLI completes in6.60s with Rayon1 and8.35s with Rayon4 (both on the same four
available CPUs). It proves neither a useful parallel speedup nor that pool
width explains the earlier804s preparation outlier. It generated no rules;
do not present these figures as a five-loop solve. Raw evidence is in
`TMP/tide-preparation-pool-control.QVmvFD/`.

Native candidate metadata, generation reports and checkpoint identities retain
the rank. Resuming a checkpoint with another rank is rejected. The cold native
loader restores the bound even for a zero-only program. Starting integrals
outside the declared rank are rejected before memoization access or mutation.
Successors above the input rank are **not clipped**: every required descendant
must still have an applicable rule or an explicitly declared terminal.

## How nonlinear exceptions are handled

The ordinary exact geometry path is unchanged. If it reaches a genuinely
unsupported nonlinear branch and a rank was supplied, it fixes one still-free
inactive original coordinate to each possible value allowed by the remaining
total numerator degree. Existing Symbolica substitution, affine admission,
factorization and ideal normalization then simplify each child before further
splitting. Already fixed negative powers consume the bound. Positive axes are
never assigned an arbitrary value or bounded by this fallback.

All children share the same geometry work limits. A failed child makes the
entire intersection incomplete; unsupported positive-power geometry, native
algebra errors, compact-index overflow and exhausted budgets are not empty
sets. The returned equality domains may cover more points outside R, but their
claimed completeness is only their intersection with the stated rank bound.
Raw nonlinear rule guards remain intact for exact concrete application.

This service does not introduce a Diophantine solver, interpolation or rational
reconstruction kernel. Polynomial operations use the existing Symbolica API;
the new code controls a finite case queue in original integral coordinates.

## Dependency inspection and limits of the claim

`CandidateReducer::trace_targets(targets, CandidateTraceLimits::default())`
follows exact applicable rules and returns all encountered uncovered keys,
explicit terminals, visited zero entries and maximum observed numerator/dot
degrees. It uses the existing guarded candidate evaluator, not another reducer.
Both input entries and distinct reachable keys are bounded; all entries share
the existing application, pending-frame and exact-arithmetic budgets. The
decomposition cache is neither read nor modified.

Only absence of an applicable rule is retained as an uncovered frontier.
Malformed formulas, failed source conditions, non-descending applications and
resource failures still abort. This trace does not independently replay source
identities. Frontier keys are not automatically masters. Since coefficients
are not back-substituted across distinct paths, some frontier terms might later
cancel; the report is a work list, not a count of independent residuals.

Successful generation is still a **candidate program**, not a closed artifact.
In particular, bounded numerator entries with arbitrary dots can have
intermediates of larger numerator degree. A proof or complete reduction must
account for those successors. Current unrestricted and total-excess certificate
installers therefore reject rank-scoped candidates rather than relabeling their
scope. The public rank option is a generation/application capability, not a
claim that all five-loop families have already been solved.

## September 21 validation and five-loop pilots

The release core suite passes **2,360 tests**, with zero failures and 32
existing ignored tests. This includes seven bounded-intersection tests and
fourteen successor-trace tests; these focused counts overlap the full suite.
An independent public-API harness also reproduces the seven intersection
tests against the optimized production library. Its assertion harness was
unoptimized and its runtime is not a solver benchmark.

The application gate passes all 147 private unit tests. All 15 CLI candidate
tests pass five consecutive times against the same optimized CLI binary.
An initial test-harness broken pipe was fixed without changing assertions:
the child can correctly reject arguments before reading stdin. All 42 Python
API tests pass with the optimized extension and four available CPUs. A prior
one-CPU harness incorrectly prevented the two multicore tests from launching;
that failed receipt is retained, and the corrected run changes only affinity.
An independently built unoptimized thin Python binding, linked to optimized
native application/core libraries, also passes all 42 tests. These are
correctness gates, not interchangeable solver timings.

The captured five-loop nonlinear conjunction is checked against the complete
original-coordinate negative-degree simplices: 3,003 points at R=10 and
53,130 at R=20. The admitted exceptional loci contain three and five points,
respectively. This verifies that specific geometry operation, not coverage of
the family or its descendants. K1 and K3 public CLI smoke runs also succeed
at both ranks and with one and two workers.

The first natural-order five-loop six-line/downset pilot uses the exact CLI
example above. With sparse exact materialization it reaches 248 observed rule
hits and case 249, but the ten-minute supervisor expires during exact
elimination before any of the seven nonzero sectors completes. The last event
starts row 511 of 520 in a frame with 1,948 integral columns and 48,019 stored
upper-row nonzeros. Those partial hits are not a saved candidate program.
Actual process wall time is 612.73 seconds, CPU time 600.68 seconds, and peak
RSS 343,316 KiB. A brief late-case sampling profile was attached, so this is
an instrumented diagnostic, not a clean performance comparison.

A same-input reconstruction repeat expires earlier in the common preparation
phase: 610.89 seconds wall, 610.78 CPU seconds, 70,432 KiB peak RSS, no sector
started. Backend selection has not yet occurred at that point. It therefore
does **not** measure reconstruction performance or establish a reconstruction
failure. Fresh small K3 controls succeed with both backends. The preparation
discrepancy requires diagnosis before comparing the two five-loop timings.

Both serial attempts return timeout status 124 and produce no complete
candidate bundle. A later six-worker reconstruction pilot on the same input
uses a one-hour deadline and a 16 GiB address-space limit. It stops at that
deadline (status 124), completing three of seven nonzero sectors and saving
1,234 rules and three
residuals in durable native checkpoint shards. These are three labelled
five-line presentations, not three inequivalent graph classes or the full
five-loop census. Common preparation took about 804 seconds. A separate
fresh-process load of one shard followed by five corner/dot/numerator canaries
passes: 98 reachable entries, 97 rule applications, no uncovered target and
one declared terminal. All five exact reductions and their memoized repeats
succeed. The check takes 7.58 seconds, including 7.52 seconds to load; it
does not replay source identities or prove coverage beyond those entries.
A second cold shard check passes another five entries and their cache repeats:
116 reachable keys, 115 exact applications, no uncovered target, one terminal,
7.62 seconds wall. Both checker processes use an unoptimized generic assertion
harness linked to optimized native libraries, so these are diagnostic timings,
not production application benchmarks.

The campaign's measured wall time is 3,600.00 seconds, CPU time 12,102.89 seconds
(12,067.45 user plus 35.44 system), and peak RSS 6,351,772 KiB. At termination,
the coalesced last event is sector 14339, case 319, reconstructing an exact
frame with 469 source rows, 1,498 integral columns and four effective variables.
Some coefficients need thousands of probes and three primes. The parent had
advanced beyond its earlier case-256 exceptional-geometry phase; the log is
coalesced across workers and is not a complete per-case trace. Its 2,375
observed rule hits include unfinished-sector work and must not replace the
1,234 durably saved rules in the reported result.

A 20-second userspace sampling profile across the live process collects 3,871
samples with no reported lost samples. Native integer multiplication, polynomial
division and Groebner arithmetic appear prominently. It is a local late-phase
sample, not a whole-run attribution; the campaign is consequently instrumented,
not a clean solver-throughput comparison. No complete seven-sector candidate
bundle is claimed. Original CLI/input hashes still match after termination.

Positive-power and above-entry-rank successor coverage remain open; terminal
minimization, numerical-master lookup and Vakint five-loop integration have
not begun. Evidence is retained locally in
`TMP/rank-scoped-gate.DWKbBo`, `TMP/rank-public-api-tests.xvfStw`,
`TMP/tide-natural-rank10-public.q6yDHY`, and
`TMP/tide-natural-rank10-reconstruction.2VscxW`,
`TMP/tide-rank10-six-workers.Z0fPdr`, and
`TMP/tide-r10-shard-canary.KwfDWh`.
