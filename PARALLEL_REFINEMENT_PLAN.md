# Genuinely parallel exact master refinement

## Objective

Make assisted exact `refine` use its configured compute budget effectively,
without weakening proof authority, dropping auxiliary equations, losing proved
maps on interruption, or changing any running production campaign. In particular,
the numerator-symmetry provider must not force all subsequent ordinary IBPs
through a serial fallback. Demonstrate correctness and measured scaling rather
than equating configured threads or busy cores with speedup.

## Starting evidence

The frozen 22 October refinement has 128 configured workers but approximately
one observed active core. `TerminalRelationExecutor::step_with_provider` delegates
to the serial session method. The app chooses that method whenever assistance
is enabled, even after all 961 numerator identities have been processed.
Approximately 1.36 million provider queries, mostly known-empty for this finite
map, accompany 0.64 million ordinary IBP rows. Dashboard `preparing` is a missing
stage fallback, not actual preparation. Read-only live sampling found exact
polynomial division/GCD work. The matrix contains approximately 607,000 independent
rows and 1.45 million auxiliary columns; few terminal eliminations do not imply
little elimination work. Evidence: `TMP/refinement-serial-path-audit-20261022/`.

## Implementation

1. Introduce an explicit immutable finite equation provider backed by the actual
   complete prepared row map. The app may use it only when every composed
   component is finite (currently numerator/circuit providers without dynamic
   saved-rule assistance). Derive its support from verified prepared plans, not
   heuristic observations. Preserve existing bindings, certificates and guards.
2. Avoid queueing or querying keys outside that declared finite domain. Matching
   old checkpoints may discard only requests proven empty by that contract;
   never discard accepted rows, pending nonempty equations, guards or cursors.
   Preserve original checkpoints and validate cold resume with changed workers.
3. Connect assisted ordinary work to the existing work-stealing source-row and
   frozen-U block-reduction engine. Keep mutable dynamic provider calls ordered,
   with explicit barriers. Workers return all independent residual rows,
   including auxiliary columns, for a deterministic exact global merge.
4. Remove the measured structural risk of cloning the entire global column
   inventory and rebuilding a million-entry map for every small batch. Reuse
   immutable existing offsets and represent only window-local additions.
   Continue using Symbolica for exact arithmetic and sparse elimination; no
   custom CAS or reconstruction kernel is introduced.
5. Report actual execution stages for finite and dynamic assistance, truthful
   progress and query filtering, while keeping observed CPU distinct from the
   thread budget. Do not display `preparing` when no preparation stage exists.
6. Keep all algorithms loop-count/topology-generic. Expose operational batch
   controls rather than hard-coded solver ceilings. No host-wide changes,
   production signals, new IBP generation campaign, or numerical master work.

## Delegation

- `parallel_refine_core`: core provider/executor/indexing and focused core tests.
- `symmetry_five_loop_validation`: app composition, finite/dynamic selection,
  exact authority preservation and progress wiring.
- `modular_refine_integration`: Python dashboard and UI regression tests.
- `fresh_search_acceptance`: independent correctness/interrupt/restore acceptance
  and matched release benchmark fixtures/harness.
- `modular_final_math_audit`: independent adversarial mathematics/code audit;
  does not approve its own implementation.
- Root: orchestration, integration, build/resource coordination, profiling,
  final verification, documentation, frozen delivery, commit and push.

## Correctness gates

- Complete fixed-source span, terminal coefficients and conditions agree between
  serial and parallel; identical flag-off behavior where promised.
- Finite provider composition is complete, unknown keys are skipped only under
  its authority, dynamic providers retain ordered calls, and malformed or changed
  bindings/certificates fail closed.
- Cross-block relations through shared auxiliary columns are retained. No shard
  may export only its terminal relations or declare its unresolved leaves masters.
- Cancellation/error at preparation, block and merge boundaries retains an
  authenticated committed prefix; native round trips, resume, changed worker
  counts and old-checkpoint activation preserve all proved maps and guards.
- Test fresh numerator assistance, mixed assistance, inherited refinements,
  empty provider lookups and actual missing-stage dashboard behavior.

## Performance and delivery gates

### Evidence-triggered second pass

The first hot 64-row pairs reproduce a 2.67x six-worker solver gain, but a matched
256-row cohort exposes the next bottleneck: 109.180s serial, 81.995s at six,
56.689s at sixteen and 61.585s at thirty-two workers. Ordered merging consumes
about 27.7s, nearly half the best parallel time. Shrinking blocks to increase
busy cores costs more CPU/RAM and is slower. These negative results are retained.

Therefore amortize whole-matrix column shifts with one preflighted, block-atomic
commit using existing Symbolica `add_cols` and `add_matrix`. Stage CSR before
mutation, preserve serial final column order and original support/conditions,
advance all cursors together, and checkpoint only complete blocks. Conservative
grant/representation misses use the legacy per-row path, not a new rejection.
Panic/OOM does not imply rollback; restore the last durable checkpoint. Test
native equivalence, cancellation and constrained-budget fallback explicitly.
Also compare several short windows against one larger window over the same
source prefix, since stale-U redundancy is not solved by adding cores.

The final build must match the old production `capacity-dispatch` feature
profile. The first standard-default prototype exposed six pre-existing
finite-source fixture/storage assumptions (physical arity 1 vs capacity 4), not
parallel mathematics failures. Do not silently change that storage convention
or weaken assertions to hide the mismatch. Repeat final native/runtime gates
and performance measurements on the actual delivery graph.

Use optimized matched binaries and fixed CPU placement, respecting the current
campaign's reservation. Compare serial, six-worker and larger 16/32-worker
budgets on representative lower-loop controls and an isolated five-loop hot
window with substantial existing U. Separate restore/setup from solver work,
report total wall/CPU/RSS, row counts, matrix fill, query overhead and merge costs.
Use bounded pilots, not an unsolicited full campaign. If a candidate is slower,
profile the actual bottleneck and apply one narrow correction before retesting.

Freeze and publish a tested optimized executable only after implementation,
independent audit, correctness gates and reproducible measurements. Report
remaining serial costs and measured scaling honestly. Provide the exact safe
stop/resume command for the user's existing `(2,0)` round if compatibility is
validated. Commit/push coherent milestones on the isolated feature branch with
the requested ValentinHirschi identity; keep all generated states, benchmark
outputs, references and license material untracked. Update `CODEX_PROGRESS.md`
after material results and at final handoff.
