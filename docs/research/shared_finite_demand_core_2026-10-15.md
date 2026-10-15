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
