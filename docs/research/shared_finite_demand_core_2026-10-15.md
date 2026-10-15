# Completed-work reuse for finite demand

## Delivered core slice

The native routed reducer now supports an exact selection census on its
parallel path and process-local reuse across completed batches. It uses the
existing worker kernel and retains the ordinary tracing APIs and defaults.
This is not yet a durable saved-campaign backend or a new R6 closure result.

`RoutedCandidateReducer::trace_targets_completed_with_entry_admission_and_observer`
returns a `CandidateRoutedCompletedTrace` only after the initial batch drains
successfully with no frontier and a complete census. The capability borrows
that exact immutable reducer and cannot be constructed from public counters,
arbitrary keys or a deserialized flag. Its `trace_batch` method can then reuse
earlier completed Work.

Work identity retains the physical integral, Route/Apply phase and literal
owner. Starting admission and existing edge/descent checks precede reuse.
A hit is a dependency on an already closed reduction graph, not a master,
zero or cached coefficient. A starting key previously encountered only as a
descendant must still pass the new starting-domain check.

Each batch runs into a separate delta. Only a complete, frontier-free delta
may enter the cumulative membership and raw terminal/zero/selection census.
Quota checks, overflow checks and allocation reservations precede the semantic
commit. Cancellation, a frontier or a failed batch preserves earlier closure.
The final observer can cancel or panic without partially committing the batch.
Downstream terminal refinements are not folded into this raw closure state.

## Accounting and limitations

Native operation budgets apply **per batch**. Retained completed Work and
census grants are cumulative. Input occurrences are not globally distinct
starting integrals; a later input may already exist as an earlier descendant.
Batch reports contain only new work and separate completed-baseline hits;
the capability exposes the cumulative exact sets/census.

The merge payload grant includes conservative membership and staged census
payload, not allocator/hash-table slack, prepared programs, caller-retained
reports or total process RSS. The current in-memory merge stages cumulative
terminal/zero/census metadata. Its cost must be measured separately; it is not
the final bounded-disk implementation. Ordinary phase counters and new completed
Work membership must never be confused with complete requested-scope coverage.

No persistence, source publication, coefficient back-substitution or physical
scope expansion is added by this slice. Those continue through the existing
application engine and the subsequent integration plan in
`SHARED_FINITE_DEMAND_PLAN.md`.

## Validation

Audited source slice: `8d4e1c10e87fd55ca0edb3bc41e76874389559b0`.
Optimized frozen executable:
`TMP/releases/20261015-shared-finite-core/rustred-core-tests`.
SHA256: `c976fbd60615683b77d222af7cadc0702abbd64ee06302a684e7edb5d1dbe3de`.

- Corrected release typecheck passed in 83.225 seconds. The initial check's
  sole failure was a new test's private-helper access; its receipt is retained.
- Release native build passed in 832.666 seconds; compilation is not solver time.
- All **542 candidate-reduction tests passed**, none ignored, in 0.96 seconds
  natively (1.168 seconds under the runtime guard).
- The 67 shared-campaign tests are a subset of those 542, not additional tests.
  Root independently reran these 67 from the frozen binary: all passed, none
  ignored, in 0.04 seconds.
- Independent source, raw runtime, binary and dependency review passed; the
  audit rechecked 1,878 source/vendor files and three dependency/config bindings.

New tests cover serial/four-worker equality, sequential-batch versus whole-union
census parity, baseline-only requests, admission-before-hit, unresolved and
failed descendants, cancellation/panic rollback, owner/phase conflicts, resource
refusals and counter overflow. The existing affine-route test also verifies
retained raw zero/terminal sets across reuse. These are correctness regressions,
not a full-family performance comparison or the entire core test suite.

Evidence:
`TMP/shared-finite-core-candidate-runtime-20261015/independent-audit.json`.
The next performance gate is the complete cumulative R0+R1 input scope, with
all 232 declarations, followed by persistence/publication lifecycle controls.
Neither this slice nor the earlier R0 sharing win establishes R6 feasibility.

## Durable exact-window lifecycle: native correctness gate

The follow-up now joins an actual input window to its native completed delta
and atomic checkpoint transition. It is integrated as private application
building blocks, not yet exposed as a production saved-campaign backend.

- A prepared execution binds the owner payloads, source conditions, routes,
  ordering and raw terminal convention actually loaded. Backing-store reuse
  is explicitly trusted-generated, not an independent replay proof.
- Completed Work pages preserve full key, Route/Apply phase and literal owner.
  Exact lookup falls back to validated disk pages after eviction; missing,
  corrupt or resource-refused pages cannot be treated as absence. Blocking
  storage reads do not hold the scheduler's global mutex.
- A sealed native delta is produced only after a globally completed,
  frontier-free batch. It retains the exact predecessor and reducer identity.
  Stale deltas cannot be attached to a later checkpoint.
- The private backend selects the next exact `PrefixWindow`, passes precisely
  its targets and explicit admission to that reducer, and commits precisely
  its successor cursor with the delta. Callers cannot supply arbitrary starts
  or cursor bytes to this committing operation.
- Cold restore checks the captured generation, scope and current cursor using
  the native count/extrema machinery. It does not replay historical windows.
  Appended requests preserve every original row; a newly nonempty rank region
  is an appended row, not an edit to an earlier closed region.
- A pending initial generation exists before the first trace. Errors and
  cancellation retain the last committed cursor. Encoded output must be
  readable by the bounded native decoder before CURRENT can be advanced.

Source slices: durable core/application `174a63e7`, fast cursor `127bd1e4`,
exact-window backend `a48a991d`. Main integration is byte-identical in the core
and app subtrees to the tested combined source; only the additive module
registration required conflict resolution.

Core native validation passed **550 distinct tests**:548 candidate-reduction
tests, including six new generated-baseline tests, plus two persistence tests.
The separate application gate passed **148 executions /147 distinct tests**,
none failed, ignored or skipped, in46.730s supervised:

| Application group | Executions |
|---|---:|
| Exact window backend | 6 |
| Fast trusted cursor restore | 6 |
| Prefix windows | 12 |
| Shared diagnostic runner | 9 |
| Aggregate policy | 2 |
| Storage, including decoder allocation limits | 24 |
| Prepared identity | 3 |
| Actual files and cold re-preparation | 2 |
| Existing finite-query lifecycle | 6 |
| Existing finite replay | 48 |
| Entry admission | 3 |
| Inventory | 22 |
| Existing shared tracing | 5 |

All groups drained without a resource stop. Independent source and runtime
reviews passed. Core compilation took557.418s and combined app compilation
1168.766s; those are build costs, not solver timings.

Frozen combined app binary SHA256:
`bec26f999e44019b9492673aeb7bee52f96a69a5b800ad818630e2890bead0c2`.
Receipts:
`TMP/finite-demand-durable-core-candidate-runtime-20261015/independent-audit.json`
and
`TMP/finite-demand-window-backend-20261015/TMP/runtime-backend-tests/independent-audit.json`.

The real full-R1 in-memory result is documented separately in
`shared_finite_r1_pilots_2026-10-15.md`. Its1.11million zero keys exceed this
first durable format's64MiB state/decoder limit. Therefore these small native
tests do **not** establish full-R1 persistence, full-R6 capacity, CLI delivery
or application publication. The measured next change is paged raw census and
hot prepared-generation reuse, followed by a real-R1 cold-resume gate.

## Paged census and durable diagnostic: integrated native gate

Main commits `5425c5d2` and `c68b789d` integrate the independently tested
`80441e77` and `1659cfd4` slices. The core/app trees are byte-identical to the
tested combined source. Typed terminal and zero pages are separate from closed
Work pages. A bounded merge streams their exact union; summed run references
are not reported as distinct terminals or zeros. Prepared generation tokens
avoid repeatedly classifying and copying the entire cumulative raw census
after each hot commit. Cold full-census checking remains explicit.

The input-steered diagnostic uses the real durable Session, not the older
in-memory runner. It supports a planned stop after committed windows and
fresh-process resume, reports effective resource grants and backing-store
statistics, and streams complete keys for independent comparison. It remains
test-only: no public campaign command or publication path is enabled by this
commit alone.

The optimized native gate passed **164 executions / 163 distinct tests** in
44.533 seconds, with zero failures, ignored tests or skipped-license markers.
This includes 32 storage tests, eight backend tests, six actual-disk/native
tests and the existing prefix, finite-replay, admission and inventory controls.
The real 1000-zero/tiny-state/cold-all-hit test checks that raw data no longer
has to fit inside the small mutable state frame. All process groups drained;
the independent auditor checked the frozen executable and source bindings.
Compilation took 1247.845 seconds, separately from runtime.

Frozen binary SHA256:
`cec6ef804e068c622af35ba55444ef86dbdcb43f1b424867a17976a5fb52755e`.
Evidence:
`TMP/finite-demand-paged-census-20261015/TMP/runtime-paged-tests/independent-audit.json`.
The full R1 disk/resource pilot, required publication provenance, public
run/extend integration and R6/application acceptance remain unfinished.

## Exact prefix-window adapter: validated follow-up

Source slice `a1476403c77a2d036b4bb7d85b55ac541c787077` adds the app-side
bounded enumeration adapter. It uses native domain equivalence, Symbolica
integer counts and existing composition iteration. It partitions a whole
admitted envelope into disjoint complete windows, retaining a small DFS cursor
instead of materializing all starts or resuming with a long iterator skip.
Unsupported geometry is an explicit refusal, never a filtered approximation.

Tests check all eight three-axis support patterns at several window sizes,
correlated A/R/D bounds, singleton and empty domains, physical-versus-storage
arity, overflow/resource refusal, interrupted proposals and changed/skipped/
repeated cursor transitions. A successful enumeration transition is not a
closed reduction: the caller may commit its successor only with the closed
batch. The present `audit_restore_cursor` replays prior metadata transitions;
it is **not** the planned fast trusted-generation restore.

The combined core+prefix app consumer check passed in 91.250 seconds. Its
optimized native build took 1,973.067 seconds, separately from runtime testing.
Frozen executable:
`TMP/finite-prefix-windows-20261015/TMP/frozen-app/rustred-app-tests`, SHA256
`def13dbf582ea4f80dc08c898da5290a829a88aa29d3803cade8137e0871b267`.

All six selected runtime groups passed: 12 prefix tests, 48 finite-replay,
6 finite-query lifecycle, 3 entry, 22 inventory and 5 shared-trace tests.
That is **96 executions of 95 distinct tests**, with zero failures, ignored
tests or explicit skips. Total supervised runtime was 23.977 seconds; all
process groups drained without a resource stop. Root independently reran the
12 prefix tests from the frozen executable; all passed in 0.01 seconds.

Independent audit checked the raw groups, exact test lists, binary and all
3,444 source/vendor/config paths. Receipt:
`TMP/finite-prefix-windows-20261015/TMP/runtime-app/independent-audit.json`.
These validate enumeration and existing lifecycle regressions; they do not
claim that the new backend itself already supports durable campaign resume,
publication or the complete R1/R6 workload.
