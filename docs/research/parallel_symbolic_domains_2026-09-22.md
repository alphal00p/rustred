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

The identical-policy six-worker diagnostic has been launched separately. Its
measurement is pending; there is no completed serial/parallel speedup result.
Neither a resource-censored prefix nor worklist exhaustion with unresolved
guards establishes full R10 closure. The next guarded-application and
excluded-conjunction changes remain unvalidated work in progress and are not
part of the release test counts above.

Local evidence: `TMP/symbolic-all67-parallel-controls.vrST2S/serial/`, including
`summary.json`, `whole.time`, input hash checks, and the native/supervisor
reports under `shared-owner-campaign.0axcvvnv/`. The frozen executable SHA-256 is
`dafa29c6d259973a2a023876ccb2a925b6c79ca379802bb56bb88d792169c586`.
