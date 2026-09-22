# Bounded parallel symbolic traversal — September 22

This slice advances the shared five-loop R10 campaign without regenerating its
saved rules. It does **not** establish full R10 closure. The last measured local
census resolves 60 of 67 graph classes and retains 81 unknown guard regions.
The preceding shared control stopped on its first owner's one-million-cell
RHS allowance after 702,384 successors and 702,253 reused requests. That prefix
is documented in [the shared-domain report](shared_domain_index_2026-09-22.md).

## Implemented changes

The symbolic worklist now supports bounded parallel inspection of one immutable
prepared owner snapshot. Workers borrow its native algebra and programs; they
send small domain/count/diagnostic records, not cloned Symbolica expressions.
Each worker retains at most one queued chunk and one local chunk, flushing at
64 logical events or a 256 KiB logical payload. An additional just-converted
descriptor is separately size-checked. These limits are not a physical RSS
prediction; native scratch, allocator overhead and coordinator state remain
separately monitored.

Successors are admitted in stable domain-ID and callback order. Completed
runs therefore preserve serial publication and deduplication. This can block
later workers behind an expensive earlier domain; neither six workers nor
fifty workers is a speedup claim. The TTY dashboard and structured events now
expose active, blocked and finished-but-uncommitted workers, logical buffered
bytes, and attempted versus committed callbacks. Native operation totals are
reported after their inspection returns, not as counters inside a running
Symbolica call.

Cancellation, native failure, worker panic and coordinator failure stop and
join the owned workers. A later speculative failure can stop an earlier
publisher, so incomplete prefixes need not match across worker counts. Its
actual domain and bounded error provenance remain in the report. Uncommitted
work is never counted as completed coverage.

The CLI and Python wrappers expose independent aggregate event, retained
frontier and per-query RHS budgets, including event, shift-group and sign-split
allowances. Removing the former ten-million aggregate-event ceiling does not
make event storage unbounded: events are streamed, while retained frontiers
have their own admission limit. The existing public Python supervisor now
accepts symbolic `--queries` as an alternative to concrete `--targets`, using
the same affinity, 50-compute-worker ceiling, 450 GB soft/500 GB hard sampled
RSS policy and no elapsed solve deadline. Existing concrete-mode defaults are
unchanged. See the [interface](../shared_owner_domain_matching.md) and
[supervisor](../shared_owner_campaign_driver.md).

## Tighter routed rank bound

For an admitted map, let `B` be the nonnegative denominator base and let `e`
be the exponent vector of a substituted numerator monomial. Its endpoint is
`B-e`, with numerator rank

```text
rank(B-e) = |e| - sum_i min(e_i, B_i).
```

If `k` originally active denominators disappear, each consumes at least one
unit of numerator degree. A strict pinched image of an actual rank-`R` domain
therefore needs at most `R-k`. For example, R10 with two lost active axes needs
R8; an R13 successor losing one needs R12, not R9. Full mapped roots retain R,
and an unbounded incoming rank stays unbounded. Constants can lower monomial
degree and cancellations remove endpoints; neither enlarges this bound.

This is generic in arity and topology. It does not use a five-loop case table,
clip intermediate work to the input rank, or bypass source conditions.
Existing Apply/Route phases, strict support descent and zero-sector ordering
are unchanged. The core tests compare the cover against actual native
polynomial transport; separate fixtures check multi-axis pinches and
above-entry ranks.

## Validation and remaining work

- Core release gate: **2,663 passed, zero failed**, 32 existing ignored
  diagnostics. Focused route-cover and routed tests: 11 and 57 passed.
- Application release suite: **341 passed, zero failed**, comprising 260 unit
  and 81 integration tests. Focused runs include real 1/2/6-worker agreement,
  Route/Apply parity, cancellation, bounded buffering, worker failures and
  separation of speculative versus published results.
- Python steering/supervisor suite: **22 passed, zero failed**. It exercises
  symbolic and concrete modes, independent limits, affinity/address-space
  admission and cleanup of owned child processes.

Implementation, mathematical extraction and runtime evidence receive separate
agent review. Local release receipts are in
`TMP/route-pinch-rank-core.iFkyOx/` and
`TMP/parallel-symbolic-app.6rHWHy/`. The earlier application typecheck stopped
on a large `json!` macro expansion; its successor splits construction into
ordinary field assignments without changing the output or crate-wide limits.
Test-build time is not solver time.

The next measured controls use all 67 initial R10 orthants, identical saved
owners and work policies, first serially and then with six workers. They
separately record preparation, traversal, whole-process time, CPU/RSS,
attempted/committed work, reuse, failures, frontiers and rank growth. No new
campaign timing or speedup is claimed here before those runs finish. An
unresolved guard or conservative overcover is not automatically a reached
missing rule; source feedback must preserve that distinction. Independent
certification remains deferred, and later five-loop stages still wait for the
actual R10 solve.

## Full-census serial control: stopped on shared admission work

The frozen release executable has now completed its first all-67 R10 control
with **incomplete status (exit 4)**. It stopped at exactly **10 billion domain
containment checks**, not an elapsed deadline, memory failure or external kill.
The process has been reaped, and the input/executable hashes still match.
This is traversal of saved candidate programs, not regeneration of their IBPs.

| Measurement | Serial observation |
| --- | ---: |
| Prepared owner/map data | 109.231 s |
| Shared dependency traversal | 3,237.317 s |
| External whole-command wall time | 3,353.39 s (55.89 min) |
| External user + system CPU | 3,322.78 s |
| Sampled peak owned aggregate RSS | 8.241 GB |
| Completed dependency domains | 197,302 |
| Failed current domain / still queued | 1 / 61,284 |
| Admitted domains | 258,587 |
| Streamed callbacks / successors | 77,141,157 / 70,293,651 |
| Reused domain requests | 73,977,351 |
| Retained unresolved guard regions | 449 |
| Largest scheduled rank envelope | R20 |

The 449 frontiers are all local dispatch Unknowns: 74 equality regions and
375 excluded-conjunction regions. No `MissingRule` frontier is reported in
this prefix. This does **not** prove that no additional rules will be needed;
queued and unresolved work remains. These are domain records, not 449 distinct
missing IBPs, graph classes or masters.

The scheduled R20 envelope is a conservative dependency bound, not a concrete
reached rank or completion of R20 input. Broad routed domains can discard
correlations and generate further requests; unbounded positive powers also
mean concrete strict descent alone does not guarantee exhaustion of finite
rank layers. See the [rank-growth audit](rank_bounded_dependency_growth_2026-09-22.md).
Do not clip intermediates or declare them masters to make this worklist finite.

The containment allowance is the measured stopping condition. Without a CPU
profile it is not a claim that containment accounts for all, or a measured
percentage of, runtime. Exact-domain and full-orthant indexes already provide
42,156,631 and 31,516,026 reuse hits respectively; the remaining general
containment scans still reach their cap. Optimize repeated admission work and
preserve native coupled-case information before simply increasing allowances.

This is one shared-host observation. During the serial control, an unintended
Nix development-shell setup attempted derivations and failed fetching
`bash53-012`; it never reached the requested formatter or a RustRed build.
The enclosing observation window was 04:53:43–04:55:25 UTC, not a measured
101-second CPU cost. Consequently this serial timing is **not an uncontaminated
benchmark**. No such environment setup is allowed during the following control.

The identical-policy six-worker diagnostic was launched separately; its
subsequent optimization stop is recorded below. There is no completed
serial/parallel speedup result.
Neither a resource-censored prefix nor worklist exhaustion with unresolved
guards establishes full R10 closure. The guarded-application and
excluded-conjunction changes were not part of this frozen control binary or
its release test counts. Their later core validation is recorded in
[the separate milestone](guarded_owner_application_2026-09-22.md); application
integration and the reuse-stream changes remain in their own gates.

Local evidence: `TMP/symbolic-all67-parallel-controls.vrST2S/serial/`, including
`summary.json`, `whole.time`, input hash checks, and the native/supervisor
reports under `shared-owner-campaign.0axcvvnv/`. The frozen executable SHA-256 is
`dafa29c6d259973a2a023876ccb2a925b6c79ca379802bb56bb88d792169c586`.

## Six-worker control: cooperative stop for measured scheduling saturation

The six-worker attempt is now reaped with exit 4 after an explicit operator
stop for optimization, **not a timeout**. The initial 67 domains had finished;
the shared recursive work had not. It used the same executable, inputs and
native work policies as the serial attempt, on CPUs 40–45 with inner pools
set to one. No build or Nix environment setup overlapped this control.

| Measurement | Six-worker incomplete observation |
| --- | ---: |
| Preparation / traversal | 104.696 s / 395.818 s |
| External whole wall / CPU | 504.27 s / 528.68 s |
| Sampled peak owned aggregate RSS | 5.890 GB |
| Completed domains / queued | 22,874 / 38,242 |
| Callbacks / reused requests | 11,385,048 / 10,849,564 |
| Retained unresolved guard regions | 81 |
| General containment checks | 187,317,605 |
| Summed worker waiting time | 1,230.587 worker-seconds |

There is one interrupted current domain and five returned but uncommitted
inspections. They are not counted as completed work. The changed counts versus
the serial result reflect **different stopped prefixes**, not resolved guards
or a performance ratio.

During elapsed 130.052–298.134 s, 83 resource samples average **1.048 busy
cores**, ranging from 0.999 to 1.079 despite six requested workers. In the
heartbeat window 105.109–333.841 s, 225 of 228 samples report five blocked
workers and three report six. These counts use the literal inclusive cutoffs
against the unrounded recorded timestamps, not rounded displayed endpoints.
This quantifies the saturation of this prefix;
it is not a CPU profile of every native call or a claim about all future jobs.

The implementation flushes at 64 logical callbacks even when repeated requests
could occupy fewer physical records. Ordered publication therefore blocks later
workers behind the current domain, with a peak of only 420,696 accounted buffered
bytes. Native work and repeated coordinator admission remain serialized in
practice. Increasing cores without changing this traffic is not supported by
these measurements.

The next narrowly scoped optimization is bounded per-job exact request reuse,
homogeneous counted reuse records, and separate physical-record/byte versus
logical-lookahead limits. Original source, coefficient, guard and descent work
must remain unchanged. A reused request still means scheduled work, not solved
coverage. This is implementation in progress, not an established speedup.

Evidence is under `TMP/symbolic-all67-parallel-controls.vrST2S/six/`, with
`shared-owner-campaign.nhy__25f/stop-request.json` recording the operator's
reason. Both runs retain their incomplete statuses; neither is a full R10 solve.

## Tested follow-up: job-local duplicate suppression and counted streaming

The next application slice is implemented and independently reviewed. Each
native domain inspection may remember up to 4,096 exact request keys and 2 MiB
of logical key payload. A key includes phase, owner, every bound and actual
finite/unbounded rank. The first request is sent normally; only later identical
requests in that job can use a compact reuse marker. Ordered publication ensures
the first admission succeeds before a dependent marker commits. This is reuse
of scheduled work, not evidence that it is already solved. Native guard,
coefficient, source, routing and descent work still runs.

Adjacent compatible markers are counted together. Chunks are now limited
separately to 64 physical records, 256 KiB accounted payload and 65,536 logical
callbacks. Per-worker published/private buffers remain bounded. Optional cache
saturation or allocation failure falls back to ordinary admission. The logical
key allowance excludes HashSet spare capacity and allocator/native overhead;
it is not a process-RSS bound.

The release application gate passes **362 tests**, including 11 new reuse tests
and existing serial/parallel lifecycle checks. A private cache-off reference
checks the native small-family result and accounting. All **26 Python steering
tests** pass. See `TMP/guarded-reuse-app-retry.Sas0k6/` for raw test receipts and
independent runtime review. Preparation and actual five-loop scheduling effects
remain to be measured; these test results establish no full-campaign speedup.

The next full-census control must report `job_local_reuse_hits`, containment
work, logical callbacks, bounded buffering, blocked workers, CPU and RSS. Its
binary also includes the newer excluded-conjunction matcher behavior, so it
must not be presented as an isolated same-workload cache comparison with the
old stopped controls above.

## Reuse follow-up control: still serialized

The tested `fc181e4f` executable was then run on the same 67 R10 initial domains
with six workers. It was cooperatively stopped for optimization, with exit 4,
after measured waiting persisted across multiple initial owners. It did **not**
reach the containment allowance or an elapsed deadline.

| Measurement | Incomplete reuse-control observation |
| --- | ---: |
| Preparation / traversal | 104.028 s / 223.725 s |
| External whole wall / CPU | 332.39 s / 329.59 s |
| Sampled peak owned aggregate RSS | 5.752 GB |
| Completed domains / queued | 59 / 14,943 |
| Committed callbacks | 7,680,580 |
| Job-local reuse hits | 3,975,413 |
| Remaining coordinator full-orthant hits | 3,231,599 |
| General containment comparisons | 23,894,791 |
| Retained local guard frontiers | 70 |

One current domain was interrupted, and five returned worker inspections remain
uncommitted. No recursive Route jobs had been completed; these counts are an
initial-owner prefix, not all 67 inspected owners or full R10 closure. The
scheduled R13 is a conservative envelope, not a concrete reached-rank claim.

In a 90-second window spanning initial completed owners 5 through 22, the
45 resource samples average about 0.992 busy cores; all 90 corresponding
heartbeats report five blocked workers. Duplicate suppression works, but has
not removed ordered-publication waiting. More cores alone are not justified by
this observation. The remaining full-orthant hit volume motivates an immutable
snapshot of already-admitted initial domains, consulted after native validation
but before allocating/streaming redundant scheduling descriptors. This proposed
change must preserve phase, owner and actual rank, and means pending work reuse,
not completed coverage. Its benefit is not yet measured.

Evidence: `TMP/shared-all67-reuse-six.x5khKa/six/`, native receipt
`shared-owner-campaign.wsua77kz/`, frozen CLI
`89e516f425216a4fb76e34eb39bdc2055f9182ce7135a839d4d2125bb3b32655`.
No other owned native solve or compilation overlapped this control. Like the
earlier stopped runs, it is not a completed-workload timing or speedup ratio.

The user-requested production allocation remains 50 cores / 500 GB. The
previous ten-billion comparison allowance was a chosen diagnostic limit, not a
solver or mathematical limitation. Production comparison work will be unbounded
by default; finite limits remain opt-in diagnostics. Memory/storage safeguards,
counter-overflow errors and evidence-based cooperative stopping remain separate.

## Tested 50-worker preparation and a real-input regression

The next application slice shares an immutable index of actual initial full
orthants, keyed by phase, owner and finite/unbounded rank. Native applicability
and successor validation still happen first. A hit suppresses only redundant
scheduling data; it never marks pending work solved. The optional index is
bounded at 4,096 buckets / 2 MiB of logical entries and falls back safely when
full. Its hits have a separate counter from job-local and coordinator reuse.

Published/private worker chunks now each allow 16,384 physical records,
8 MiB of accounted descriptor payload and 1,048,576 logical callbacks. At 50
workers the pair accounts for at most 800 MiB; separately held incoming records,
coordinator data, container capacity, native scratch and loaded programs are
additional. These bounds preserve ordered publication, cancellation and exact
counter prefixes. They are not a process-RSS estimate or a measured speedup.

The aggregate containment-comparison policy defaults to unlimited (`None` in
Rust, `unlimited` in CLI/Python, `null` in telemetry). Explicit finite budgets
remain supported. Checked counter overflow and storage/memory admission are
separate. Explicit positive `max_domains` values no longer encounter a hidden
one-million-domain ceiling; the queue allocates incrementally.

The combined release gate passes **377 Rust tests (295 application unit tests
and 82 integration tests)** and **27 Python tests**, zero failures. Focused
domain-budget and walking tests pass 2 and 49 tests respectively. Evidence:
`TMP/fifty-ready-app.XXNQSQ/`, including independent runtime review, before/after
source hashes and frozen CLI
`7401238bb9da34301b4fbe214487aa3e3b1f3227ef5ceca58b0b3d0895073219`.
These test timings are not IBP-generation or campaign timings.

Before the 50-worker traversal, a local all-67 input check found an operational
regression in the new later-guard rejection path. It stopped in the **first**
owner, with zero completed queries, on `guard separable factor work`:
193,857,088 requested against a 64,000,000 native allowance. The reported
predicate is original denominator, batch 0 / rule 405 / term 1. Its R10 cell
contains exactly fifteen integer points; independently checking the earlier
saved result shows all fifteen assigned to that same rule. Thus this is not
evidence of a missing IBP or actual expression swell. The speculative guard
probe can encounter a broader cell before the original bounded refinement.
The correction must preserve mandatory guard/source validity and native errors,
while allowing the existing bounded path to handle this case.

This local check retained 11,658 selected pieces and 73 terminal pieces before
failure. Its zero retained unknowns is a prefix, **not** elimination of the
earlier 81 unresolved regions. It overlapped an application build on disjoint
CPUs and is not a performance comparison. Raw evidence and an independent
fifteen-point recount are in `TMP/rejection-full67-local.isowEn/`.

The prepared next full run uses 50 workers, the same saved 67-owner R10 inputs,
unlimited containment comparisons, explicit 10-million-domain storage, and
450/500 GB aggregate RSS monitoring. No elapsed deadline is introduced. Launch
remains held for the actual-input regression above; this preparation is not
itself an R10 closure result.
