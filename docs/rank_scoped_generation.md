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
uses a one-hour deadline and a 16 GiB address-space limit. At 07:07 UTC it has
completed three of seven nonzero sectors, saving 1,234 rules and three
residuals in durable native checkpoint shards. These are three labelled
five-line presentations, not three inequivalent graph classes or the full
five-loop census. Common preparation took about 804 seconds. A separate
fresh-process load of one shard followed by five corner/dot/numerator canaries
passes: 98 reachable entries, 97 rule applications, no uncovered target and
one declared terminal. All five exact reductions and their memoized repeats
succeed. The check takes 7.58 seconds, including 7.52 seconds to load; it
does not replay source identities or prove coverage beyond those entries.
Remaining jobs are still running at this dated checkpoint; no complete
seven-sector candidate bundle is claimed.

Positive-power and above-entry-rank successor coverage remain open; terminal
minimization, numerical-master lookup and Vakint five-loop integration have
not begun. Evidence is retained locally in
`TMP/rank-scoped-gate.DWKbBo`, `TMP/rank-public-api-tests.xvfStw`,
`TMP/tide-natural-rank10-public.q6yDHY`, and
`TMP/tide-natural-rank10-reconstruction.2VscxW`,
`TMP/tide-rank10-six-workers.Z0fPdr`, and
`TMP/tide-r10-shard-canary.KwfDWh`.
