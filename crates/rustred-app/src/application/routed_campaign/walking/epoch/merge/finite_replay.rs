use super::*;

pub(super) fn validate<const N: usize>(
    state: &EpochState<N>,
    r: &JobResult<N>,
    config: MergeConfig,
    attempts: u8,
) -> Result<(), Fatal> {
    if (r.kind == NativeKind::FiniteReplay) != r.finite_replay.is_some() {
        return Err(fatal("P1: finite replay kind/recipe mismatch"));
    }
    let Some(recipe) = r.finite_replay else {
        return Ok(());
    };
    recipe.validate().map_err(fatal)?;
    let domain = state
        .store
        .domains
        .get(r.parent as usize)
        .ok_or_else(|| fatal("P1: finite replay parent range"))?
        .expand();
    if config.finite_replay != Some(recipe.limits)
        || r.parent != 0
        || state.p0 == 0
        || attempts != 0
        || (domain.lower.iter().any(|&lo| lo != 0)
            && domain
                .lower
                .iter()
                .zip(&domain.upper)
                .any(|(&lo, &hi)| hi != Some(lo)))
        || state.anchors.get(r.parent).is_some()
        || r.scope.is_some()
        || r.g2.is_some()
        || r.error.is_some()
        || r.panic
        || r.error_kind != ErrorKind::None
        || r.break_reason != BreakReason::None
        || r.emitted != 1
        || r.accepted != 1
        || r.stats_events != 1
        || r.successors != 0
        || r.conditional != 0
        || r.known_reuse != 0
        || r.job_duplicates != 0
        || r.optional != [0; 3]
        || r.route_masks != 0
        || r.route_joint_pruned != 0
        || !r.frontiers.is_empty()
        || !r.refusals.is_empty()
        || r.refusals_truncated
        || !r.misses.is_empty()
    {
        return Err(fatal(
            "P1: finite replay is not the enabled whole initial domain completion",
        ));
    }
    Ok(())
}
