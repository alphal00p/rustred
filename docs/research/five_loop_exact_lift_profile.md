# Five-loop exact-lift profile: a bounded, incomplete family attempt

September 20, 2026. The full physical-root run below **did not complete**.
It uses the external [banana family input](../../examples/input/five_loop_banana.toml)
with the generic solver; no loop-count-specific strategy or relation was added.

## Workload and terminal outcome

The frozen optimized CLI from published `f49562cb` used natural ordering,
`sparse-factorized`, numerical depth zero, and nonpositive auxiliary coordinates
6 through 14. Its actual preparation found seven nonzero physical sectors,
57 scoped zero sectors, and 4,679 global zero proofs. All seven nonzero sectors
were requested; this was not a selected-sector or finite-target test.

One worker was pinned to CPU94 with nested pools limited to one. The caller
specified a 1,800-second wall deadline, ten-second termination grace, an 8 GiB
virtual-address limit, and a separate 2 GiB checkpoint-disk allowance. Final
candidate and native-algebra limits remained unchanged. No prior rules,
checkpoints, FORM hints, or certification participated.

| Observation | Result |
| --- | ---: |
| Exit | 124, deadline; TERM reported |
| Whole wall time | 1,800.88 s |
| User / system CPU | 1,768.87 / 16.03 s |
| Peak RSS | 7,923,228 KiB, approximately 7.56 GiB |
| Completed and saved sectors | 0 / 7 |
| Final candidate / report | Neither produced |

No OOM or allocation failure was reported. An address-space limit is not an RSS
limit: these numbers do not establish remaining allocation headroom or that the
cap was reached. The checkpoint directory retains only its manifest and lock;
the direct solver and wrapper are terminal. Input hashes pass. The earlier
600-second attempt, which also saved no sector, remains separate evidence.

This run overlapped correctness compilation on other CPUs and included the
profiling intervals below. Its times are **shared-load, instrumented resource
observations**, not an isolated benchmark or a paired backend speed ratio.

## What the CPU samples identify

A flat 99 Hz user-CPU recording contains 2,601 samples across approximately
30.03 seconds. A later 49 Hz DWARF-stack recording contains 475 samples across
approximately 20.05 seconds. Neither reports lost samples. They are different
intervals and must not be combined into one distribution.

| Exclusive sampled instruction bucket | Flat interval | Stack interval |
| --- | ---: | ---: |
| Native total-degree integer-polynomial multiplication | 28.14% | 39.16% |
| Symbolica dense polynomial multiplication | 27.99% | 14.95% |
| Symbolica packed heap polynomial division | 19.26% | 18.74% |
| Native BTreeMap lookup in sampled polynomial work | 10.80% | 9.68% |

All 475 captured stacks include the native factorized `SparseRowReducer`'s
`add_row` / `scatter_with_touched`, reached from exact materialization.
Factorized rational-polynomial addition and multiplication have 68.21% and
31.79% inclusive samples respectively. Inclusive descendants overlap; they
cannot be added to the exclusive percentages above. In this interval the work
is elimination arithmetic, not coefficient import, final output conversion, or
pivot inversion. This does not exclude those costs elsewhere in the run.

Outer unwinds are incomplete or unproven: the outer application entry appears
in only 55.16% of stacks. Zero reported lost samples does not prove complete
sampling or unwinding. The stack recording's summed sample periods correspond
to about 9.694 seconds, **not** measured solver CPU time over its 20.05-second
span; the difference is not explained here. Whole-process accounting comes
from GNU time, not that sum. Profiling overhead was not independently measured.
An earlier stack-recording attempt failed to map a 256-page buffer and recorded
no data. Reducing the buffer to 64 pages succeeded without escalation or any
host-setting change; both attempts are retained.

## Why factorized denominators do not eliminate this cost

The [existing factorized materializer](../../crates/rustred-core/src/solver/discovery/factorized.rs)
already retains native factorized coefficients throughout sparse elimination
and materializes only requested output rows. It does not repeatedly convert
every intermediate to ordinary rational polynomials.

Symbolica's factorized representation retains denominator factors, but its
numerator is expanded. For example, adding `p/a + q/b` still requires numerator
work corresponding to `p*b + q*a`, followed by exact cancellation checks.
The inspected native addition/multiplication paths perform precisely that
kind of multiplication and division inside sparse row updates. Native pivot
inversion is already computed once per accepted row, not once per row entry.
There is no demonstrated unused normalization switch that safely removes this
work, and no new computer-algebra kernel is warranted inside RustRed.

## Target-block control: lower memory, still incomplete

The existing `sparse-target-factorized` backend provides a useful control:
reduce only the harder/target block, solve for source weights with native sparse
linear algebra, and reconstruct the complete exact row. It preserves the same
factorized numerical tail and requires a source prefix independent in the
restricted harder/target block. A fresh full-root control used the same frozen
CLI, input, ordering and resource policy, with only the backend changed. It
reused no checkpoints bound to the other backend. It also reached its deadline:

| Observation | Target-block factorized result |
| --- | ---: |
| Exit | 124, deadline; TERM reported |
| Whole wall time | 1,800.44 s |
| User / system CPU | 1,790.02 / 6.37 s |
| Peak RSS | 513,128 KiB, approximately 0.49 GiB |
| Completed and saved sectors | 0 / 7 |
| Final candidate / report | Neither produced |

Frozen CLI, input and protocol hashes pass, both processes are terminal, and no
OOM was reported.
The two incomplete attempts have markedly different peak-memory observations,
but neither supplies completed outputs or a measured solve-time speedup. The
target control was also shared-load and instrumented; profiling overhead was
not separately measured.

A 49 Hz, 20.10-second user-CPU stack sample contains 332 samples and reports
zero lost samples. Native sparse row insertion and factorized-field inversion
appear in 97.29% of the inclusive stacks, alongside polynomial factorization.
The native factorization/Hensel-lifting subtree dominates this interval, unlike
the earlier full-row interval dominated by multiplication and division.
The local native API agrees with this interpretation:
`FactorizedRationalPolynomial::inv` explicitly factors its numerator, and the
field's inverse delegates to it. The constructor's `do_factor=false` does not
disable this later inversion. No replacement arithmetic or unproved shortcut
was introduced.

The target-only factorized materializer appears in 96.99% of the captured
stacks and the outer exact-materialization dispatcher in 89.16%, not all of
them. The summed sampling periods are about 6.776 seconds over 20.10 seconds
of recording, not measured whole-process CPU time; that discrepancy remains
unexplained. These observations do not prove one pivot consumed the entire run,
nor do they establish the exact frame dimensions or a missing algebraic rule.

This control can establish whole-workflow feasibility. Without capturing and
comparing the actual exact frame, it cannot isolate which frame or omitted
columns explain a difference. Full exact backend parity also requires both
completed saved outputs. The previous completed cube-parent comparison showed
a target-block regression, 395.338 versus 75.492 seconds of solver time; there
is no automatic backend switch or general speedup claim. See the
[factorized-field study](factorized_coefficients.md).

The existing semi-numerical route remains a separate option, but
combines reconstruction with full ordinary exact replay of the selected target
row; enabling it does not
remove all exact elimination. The
[source-weight validation implementation](reconstruction_exact_validation.md)
is now an opt-in Rust backend with 2,299 passing release core tests. Subsequent
TIDE-input diagnostics complete a factorized selected sector and exactly match
the sparse result; a connected-sector attempt stops at the same nonlinear
exceptional geometry with both backends, after 66.985/81.514 seconds respectively.
These are different external inputs from the older banana profile above, not
completions or speed ratios for that earlier campaign. There is still no
completed connected five-loop family or universal source-weight speedup. The
separate observability
change has passed its release frontend gate: existing scalar frame/row/nonzero
counters now reach CLI progress rather than all being collapsed to `exact
lift`. It introduces no new algebra and cannot observe inside a long native
call without another callback. Neither change retroactively adds frame-level
information to these older profiles.

## Evidence

### September 21: rank-10 TIDE subfamily, twelve unsaved sectors

A later **different-input** diagnostic samples the live four-worker TIDE
subfamily 30231 continuation with natural ordering, `sparse-factorized` and
`retain-rank-finite` at R=10. It has admitted 172 previous sectors and saved
14 more; 12 of 198 remain unsaved. This is not a completed family, an isolated
case benchmark or a comparison against the older banana experiments above.

A 20-second, 49 Hz user-space sample on 2026-09-21 records 3,875 samples and
reports zero lost samples. Leading exclusive shares are native packed
polynomial division **18.09%**, GMP basecase multiplication **9.69%**, native
polynomial BTree search/insertion/removal **8.10/6.89/5.62%**, dense integer
polynomial multiplication **6.53%**, and integer-ring exact division **4.37%**.
Resolved stacks connect that arithmetic to factorized sparse row reduction;
the outer materializer/reducer appears in only 959 of 3,875 captured blocks,
so complete outer unwinding is not established. Do not sum inclusive stack
costs with these exclusive percentages or call this a whole-run breakdown.

Coalesced progress shows one frame advancing from row 2400 to 2406 of 2456,
with roughly 223k–227k upper nonzeros; another simultaneous frame is at row
3810 of 4130, with 465,189 upper nonzeros and 10,644 integral columns. The
process-wide sample cannot assign its shares to a particular worker/case.
These are ongoing exact-arithmetic/fill costs, not a new demonstrated missing
IBP or exceptional-domain failure. The recording spans only 20 seconds and
does not establish how long either frame or family will take to finish.

The continuation is consequently an instrumented resource diagnostic.
Evidence: `TMP/tide-r10-root30231-profile.4nE5LU/RESULTS.md`; the profiler/report
clock interval is 09:23:56–09:24:21 UTC, including report generation. No native
algebra kernel, exactness shortcut or automatic backend switch was added.

### Short ordinary-sparse control

A separately admitted five-minute control reused the identical published CLI,
external family, ordering and physical-root request, selecting existing
`sparse` instead of either factorized backend. It did not wait for the pending
frontend/progress rebuild and therefore has only the old coarse callbacks.
No source changed and no profiler was attached. The same CPU94/one-worker,
8 GiB address-space, 2 GiB spool and final-output policies applied.

It also remained incomplete: status 124, **302.26 s wall, 298.80 s user,
3.26 s system, 88,780 KiB peak RSS**, with zero saved sectors or final bundle.
Hashes pass. Observations reached depth two (64 seeds, 1,575 discovered rows),
followed by exact-lift callbacks through 271.5 s. These are coarse discovery
counts, not the selected exact frame size, completed rules or a proof that the
same frame/pivot was used by the other backends. The shorter deadline, ordinary
shared numerical lane and shared host preclude a completed-workload speed ratio.
This one-shot diagnostic is retained in
`TMP/five-loop-banana-sparse-control.XaLf5i/`; it is not automatically resumed.

### Earlier run and profile receipts

The complete run, command, resource receipt and independent terminal audit are
in `TMP/five-loop-banana-followup.viv69F/`. Frozen CLI SHA256:
`60a4bc6f8ab89175117ee23f71c258c2a5c23d782eff36a50642475b507fb462`.
Input SHA256:
`aedb3886ca8032eafbf4fb4717033c2c6a1d5da810b0a0fcdf3ad87953dfd956`.
Profiles are in `TMP/banana-live-profile.bdDIeY/`,
`TMP/banana-live-stacks.jt4kO4/` (failed), and
`TMP/banana-live-stacks-small.nOE5KJ/`. Independent measurement and native API
reviews are `TMP/banana-live-profile-independent-audit-2026-09-20.md` and
`TMP/factorized-exact-lift-hot-path-audit-2026-09-20.md`.
The terminal target-control receipts are in
`TMP/five-loop-banana-target-control.hPo90N/`, its profile in
`TMP/banana-target-live-stacks.qTFPC0/`, and its independent interpretation in
`TMP/banana-target-profile-outcome-independent-audit-2026-09-20.md`.
These local evidence directories are not shipped artifacts or closure proofs.
