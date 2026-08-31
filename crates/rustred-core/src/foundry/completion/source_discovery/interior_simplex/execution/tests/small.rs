use std::sync::Arc;

use crate::foundry::artifact::{
    derive_one_loop_unit_mass_tadpole, derive_two_loop_unit_mass_sunset,
};
use crate::foundry::completion::UncoveredPartition;
use crate::foundry::completion::source_discovery::interior_simplex::{
    InteriorSimplexExecutionError, InteriorSimplexOutcomeTelemetry, InteriorSimplexProbeExecutor,
    InteriorSimplexReplayRetention,
};
use crate::foundry::completion::stratum::ImmutableOwnerSnapshot;
use crate::identity::ParametricIbpGenerator;
use crate::sector::{Mask, OrderingPolicy};

use super::helpers::{
    assert_structural_accounting, bounded_execution_limits, complete_ordinary, declared_probe,
    execute, lattice_box, one_scope_plan, summarize,
};

#[test]
fn one_loop_tasks_execute_independently_and_deterministically() {
    let artifact = Arc::new(derive_one_loop_unit_mass_tadpole().unwrap());
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = bounded_execution_limits();
    let owners = ImmutableOwnerSnapshot::try_from_closed_artifact(
        artifact.clone(),
        limits.scheduler.campaign.stratum,
    )
    .unwrap();
    let sector = Mask::try_new([true]).unwrap();
    let partition = UncoveredPartition::new(vec![lattice_box(&[0], &[None])], 0);

    let first = execute(
        one_scope_plan(101, &sector, &partition, 1),
        &generator,
        &completed,
        owners.clone(),
        limits,
    );
    let repeated = execute(
        one_scope_plan(101, &sector, &partition, 1),
        &generator,
        &completed,
        owners,
        limits,
    );
    assert_eq!(first.tasks().len(), 2);
    assert_eq!(first.plan_epoch_ordinal(), 101);
    assert_eq!(first.interior_margin(), 1);
    assert_eq!(first.polynomial_degree_ceiling(), 1);
    assert_structural_accounting(&first);
    assert!(matches!(
        first.tasks()[0].probes()[0].outcome(),
        InteriorSimplexOutcomeTelemetry::Replayed {
            exact_support: InteriorSimplexReplayRetention::UnsupportedEpochBoundCircuit,
            ..
        }
    ));
    assert_eq!(summarize(&first), summarize(&repeated));
}

#[test]
fn two_loop_dot_ray_executes_with_the_same_canonical_accounting() {
    let artifact = Arc::new(derive_two_loop_unit_mass_sunset().unwrap());
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = bounded_execution_limits();
    let owners = ImmutableOwnerSnapshot::try_from_closed_artifact(
        artifact.clone(),
        limits.scheduler.campaign.stratum,
    )
    .unwrap();
    let sector = Mask::try_new([true, true, true]).unwrap();
    let partition =
        UncoveredPartition::new(vec![lattice_box(&[0, 0, 0], &[None, Some(0), Some(0)])], 0);

    let first = execute(
        one_scope_plan(202, &sector, &partition, 1),
        &generator,
        &completed,
        owners.clone(),
        limits,
    );
    let repeated = execute(
        one_scope_plan(202, &sector, &partition, 1),
        &generator,
        &completed,
        owners,
        limits,
    );
    assert_eq!(first.tasks().len(), 2);
    assert_structural_accounting(&first);
    assert_eq!(summarize(&first), summarize(&repeated));
}

#[test]
fn retained_identity_and_probe_payload_caps_reject_exactly_one_below() {
    let artifact = Arc::new(derive_one_loop_unit_mass_tadpole().unwrap());
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let base_limits = bounded_execution_limits();
    let owners = ImmutableOwnerSnapshot::try_from_closed_artifact(
        artifact.clone(),
        base_limits.scheduler.campaign.stratum,
    )
    .unwrap();
    let sector = Mask::try_new([true]).unwrap();
    let partition = UncoveredPartition::new(vec![lattice_box(&[0], &[None])], 0);

    let plan = one_scope_plan(401, &sector, &partition, 1);
    let key_cells = plan
        .tasks()
        .iter()
        .map(|task| {
            task.key().sector().arity()
                + task.key().box_lower().len()
                + task.key().box_upper().len()
                + task.key().simplex_offset().len()
                + task.target_shift().len()
        })
        .sum::<usize>();
    let mut key_limits = base_limits;
    key_limits.max_retained_task_key_coordinate_cells = key_cells - 1;
    let error = InteriorSimplexProbeExecutor::try_new(
        plan,
        &generator,
        &completed,
        owners.clone(),
        OrderingPolicy::default(),
        [declared_probe(&generator, key_limits.scheduler.campaign)],
        key_limits,
    )
    .unwrap_err();
    assert_eq!(
        error,
        InteriorSimplexExecutionError::ResourceLimit {
            resource: "retained task-key coordinate cells",
            requested: key_cells,
            limit: key_cells - 1,
        }
    );

    let plan = one_scope_plan(402, &sector, &partition, 1);
    let scope_bytes = plan
        .tasks()
        .iter()
        .map(|task| task.key().stable_scope_key().len())
        .sum::<usize>();
    let mut scope_limits = base_limits;
    scope_limits.max_retained_stable_scope_key_bytes = scope_bytes - 1;
    let error = InteriorSimplexProbeExecutor::try_new(
        plan,
        &generator,
        &completed,
        owners.clone(),
        OrderingPolicy::default(),
        [declared_probe(&generator, scope_limits.scheduler.campaign)],
        scope_limits,
    )
    .unwrap_err();
    assert_eq!(
        error,
        InteriorSimplexExecutionError::ResourceLimit {
            resource: "retained stable-scope-key bytes",
            requested: scope_bytes,
            limit: scope_bytes - 1,
        }
    );

    let plan = one_scope_plan(403, &sector, &partition, 1);
    let probe = declared_probe(&generator, base_limits.scheduler.campaign);
    let probe_cells =
        plan.tasks().len() * (probe.base_parameters().len() + probe.chart_coordinates().len());
    let mut probe_limits = base_limits;
    probe_limits.max_retained_probe_coordinate_cells = probe_cells - 1;
    let error = InteriorSimplexProbeExecutor::try_new(
        plan,
        &generator,
        &completed,
        owners,
        OrderingPolicy::default(),
        [probe],
        probe_limits,
    )
    .unwrap_err();
    assert_eq!(
        error,
        InteriorSimplexExecutionError::ResourceLimit {
            resource: "retained task-probe coordinate cells",
            requested: probe_cells,
            limit: probe_cells - 1,
        }
    );
}

#[test]
fn aggregate_iteration_cap_rejects_second_task_without_a_partial_report() {
    let artifact = Arc::new(derive_one_loop_unit_mass_tadpole().unwrap());
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let mut limits = bounded_execution_limits();
    limits.max_retained_iteration_records = 1;
    let owners = ImmutableOwnerSnapshot::try_from_closed_artifact(
        artifact.clone(),
        limits.scheduler.campaign.stratum,
    )
    .unwrap();
    let sector = Mask::try_new([true]).unwrap();
    let partition = UncoveredPartition::new(vec![lattice_box(&[0], &[None])], 0);
    let probe = declared_probe(&generator, limits.scheduler.campaign);

    let error = InteriorSimplexProbeExecutor::try_new(
        one_scope_plan(404, &sector, &partition, 1),
        &generator,
        &completed,
        owners,
        OrderingPolicy::default(),
        [probe],
        limits,
    )
    .unwrap()
    .run()
    .unwrap_err();
    assert_eq!(
        error,
        InteriorSimplexExecutionError::ResourceLimit {
            resource: "retained compact iteration records",
            requested: 2,
            limit: 1,
        }
    );
}
