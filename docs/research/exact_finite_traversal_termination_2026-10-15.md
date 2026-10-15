# What exact finite traversal does—and does not—guarantee

This is a conditional argument for the existing concrete routed kernel, based
on code inspection. It is not a new certification framework, a completed R6
solve, or a prediction of running time.

## Checked descent

Every surviving concrete child of an applied rule must compare strictly below
its parent in that owner's persisted integral order. The shared evaluator
checks this before scheduling or deduplication; seeing a child before cannot
hide a non-descending edge. A same-support child remains in the same literal
owner. A support-changing child must have strictly fewer positive lines.

Routing either enters an installed owner directly or uses an admitted map to
one. Routes cannot redirect installed owners. The routed root preserves the
positive-line count; emitted children either enter that owner or lose support.
Thus a path cannot alternate indefinitely between different owners at a fixed
support size.

Relevant code is in `candidate_reduction/evaluator.rs::apply_selected`,
`routed/campaign/worker.rs::child`, `routed/prepare.rs::validate_routes`, and
`preparation/shared.rs::validate_solution_order`, below
`crates/rustred-core/src/solver/`.

## Why strict comparison is sufficient here

An arbitrary strict comparator would not establish termination. The supported
orders have stronger validation:

- Legacy orders compare nonnegative total excess before their final coordinate
  ties. Each fixed-excess level has finitely many integral keys, including when
  the final tie direction is reversed.
- Programmed orders are validated declarative data, not arbitrary callbacks.
  Their ascending degree rows have nonnegative integer weights, and every
  coordinate in each sign sector has a positive weight in at least one row.
  The tuple of row values is well-founded lexicographically. Fixing that tuple
  bounds every coordinate, so its remaining tie set is finite.

The latter requirements are enforced by
`crates/rustred-order/src/descriptor.rs::validate` and used by `CompiledOrder`.
An order may put a degree row before support; that does not invalidate this
fixed-support argument, because the concrete worker separately enforces strict
support decrease on transitions out of an owner.

## Conditional termination, not guaranteed successful reduction

Assume finitely many exhaustively admitted starting keys, an immutable finite
rule/owner/route snapshot, the checks above, and completion of each local CAS
and transport operation. Along a path, support can decrease only finitely many
times. Between decreases, the path descends in a fixed owner's well-founded
order. Hence there is no infinite concrete dependency path.

Each rule and finite integer-power transport has finitely many children. A
finitely branching tree with finitely many roots and no infinite path is
finite. Exact shared-subproblem deduplication can only reduce this work.

The outcome can nevertheless be an explicit missing-rule/owner frontier,
source-condition failure, non-descent error, overflow, resource refusal or
cancellation. None can be promoted to a master or successful closure. This
argument also does not verify the IBP identities themselves or establish
independence of the declared terminals.

## Why there is still no useful ETA

Well-founded descent need not give a small predecessor set. Descendant degree
can grow on a support decrease, and branching can be large. Existing counters
do not provide a useful bound on unseen work.

The symbolic overcover walker is different: it schedules enclosing regions,
which can widen or be reintroduced. A concrete integral's decreasing order is
not necessarily a decreasing measure on those region transitions. Therefore
finite starting bounds alone still do **not** prove termination of the symbolic
worklist. The shared-union experiment measures whether avoiding that region
overcoverage and repeated concrete work helps on the unchanged input scope;
it does not transfer this argument to the running symbolic campaigns.
