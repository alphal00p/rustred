# Retain finite refinement progress across rank extensions

## Request and scope

Remove the arbitrary million-column stop from saved-campaign refinement, and
make already proved terminal substitutions from unfinished refinement reusable
by subsequent rank extensions. Do not add relation discovery to the symbolic
walker or run new Laporta work implicitly during extension. Production campaigns
remain user-controlled and untouched by development/testing.

## Implementation

1. Use the existing RAM supervisor for ordinary application runs rather than a
   fixed million-column threshold. Offer an explicit optional column cap, allow
   changing it on resume, and preserve binary input bounds and other real limits.
   Do not change coefficient arithmetic: Symbolica remains the algebra engine.
2. Add explicit `finalize-refinement`: cold-load the last valid stopped native
   refinement checkpoint, preserve its provenance and parameter conditions,
   finish only pending row rebuilds, and export its currently proved maps in an
   immutable publication. Keep the original unfinished search checkpoint.
3. Clearly distinguish completed scope coverage, usable partial refinements,
   and an exhausted finite relation search. Never turn a resource failure into
   a false completed-search claim.
4. Keep artifact/predecessor selection consistent, so subsequent `extend`
   imports these maps. Warn/refuse a transition that would silently bypass
   compatible unretained refinement progress, with an actionable command.
5. Check correctness with focused core/application tests and the public
   finalize→extend→refine workflow. Use private copies only for actual THE_ONE
   partial-checkpoint checks. No need to finish its expensive finite search.

## Delegation

- Root: resource configuration, integration, build/tests, documentation and push.
- `refinement_partial_export`: native immutable partial finalization.
- `refinement_steering`: CLI/Python workflow and pointer/cursor preservation.
- `refinement_budget_audit`: independent diagnosis and adversarial review.

## Evidence and delivery

Record whether the failed run was memory-bound or merely hit a preset ceiling;
do not infer good scaling from removing that ceiling. Confirm the exact retained
maps/conditions, source immutability and honest incomplete-search metadata.
Publish a tested optimized executable and task-scoped commit/push using the
requested Git identity. Preserve unrelated worktree changes.
