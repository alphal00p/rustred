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
