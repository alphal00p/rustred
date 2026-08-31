use crate::foundry::artifact::{
    FULL_RANK_ORBITS, canonical_three_loop_family, derive_k6_terminal_authority,
};
use crate::foundry::completion::UncoveredPartition;
use crate::foundry::completion::source_discovery::interior_simplex::{
    InteriorSimplexLimits, InteriorSimplexPlan, InteriorSimplexScopePartition,
    try_plan_interior_simplex_samples,
};
use crate::foundry::completion::stratum::ImmutableOwnerSnapshot;
use crate::identity::{ParametricIbpConfig, ParametricIbpGenerator};
use crate::sector::Mask;

use super::helpers::{
    assert_structural_accounting, bounded_execution_limits, complete_ordinary, execute,
    lattice_box, summarize,
};

fn typed_manifest_shell_plan(epoch: u64) -> InteriorSimplexPlan {
    let first_sector = Mask::try_from_indices(&FULL_RANK_ORBITS[0].representative).unwrap();
    let second_sector = Mask::try_from_indices(&FULL_RANK_ORBITS[1].representative).unwrap();
    let first_partition = shell_partition(&first_sector);
    let second_partition = shell_partition(&second_sector);
    try_plan_interior_simplex_samples(
        epoch,
        [
            InteriorSimplexScopePartition::new(
                "typed-manifest-sector-0",
                &first_sector,
                &first_partition,
            ),
            InteriorSimplexScopePartition::new(
                "typed-manifest-sector-1",
                &second_sector,
                &second_partition,
            ),
        ],
        1,
        0,
        InteriorSimplexLimits::default(),
    )
    .unwrap()
}

fn shell_partition(sector: &Mask) -> UncoveredPartition {
    let lower = vec![0; sector.arity()];
    let upper = sector
        .active_bits()
        .iter()
        .map(|&active| active.then_some(0))
        .collect::<Vec<_>>();
    UncoveredPartition::new(vec![lattice_box(&lower, &upper)], 0)
}

/// The first two typed full-rank manifest sectors are the factorized
/// path/star endpoints.  No name dispatch, support list, coefficient, or
/// expected scheduler outcome enters this diagnostic.
#[test]
fn k6_typed_manifest_shells_have_deterministic_one_epoch_diagnostics() {
    let family = canonical_three_loop_family().unwrap();
    let generator =
        ParametricIbpGenerator::try_new_with_config(&family, ParametricIbpConfig::default())
            .unwrap();
    let completed = complete_ordinary(&generator);
    let limits = bounded_execution_limits();
    let owners = ImmutableOwnerSnapshot::try_from_terminal_authority(
        derive_k6_terminal_authority().unwrap(),
        limits.scheduler.campaign.stratum,
    )
    .unwrap();

    let first = execute(
        typed_manifest_shell_plan(303),
        &generator,
        &completed,
        owners.clone(),
        limits,
    );
    let repeated = execute(
        typed_manifest_shell_plan(303),
        &generator,
        &completed,
        owners,
        limits,
    );

    assert_eq!(first.tasks().len(), 2);
    assert_structural_accounting(&first);
    assert_eq!(summarize(&first), summarize(&repeated));
    for (task, orbit) in first.tasks().iter().zip(FULL_RANK_ORBITS.iter()) {
        assert_eq!(
            task.task_key().sector(),
            &Mask::try_from_indices(&orbit.representative).unwrap()
        );
    }
}
