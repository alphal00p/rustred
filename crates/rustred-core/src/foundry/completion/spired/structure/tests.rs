use std::sync::Arc;

use crate::foundry::artifact::{
    ClosedArtifact, derive_one_loop_unit_mass_tadpole, derive_two_loop_unit_mass_sunset,
};
use crate::foundry::completion::source_discovery::{CampaignLimits, CampaignModularProbe};
use crate::foundry::completion::stratum::{
    DecoratedStratum, ImmutableOwnerSnapshot, StratumRegistryLimits,
};
use crate::identity::{
    CompletedIbpSourceRows, IntegralShift, ParametricIbpGenerator, TranslatedSourceRequest,
};
use crate::sector::{Mask, OrderingPolicy, SectorMonotoneDomain};

use super::super::{
    SpiredCase, SpiredCoordinateFace, SpiredExecutionCase, SpiredModularLimits,
    SpiredModularStreamOutcome, SpiredPreparedStreamingDiscovery, SpiredStreamingDiscovery,
    SpiredStreamingLimits,
};
use super::{
    SpiredPreparedTermRole, SpiredStructuralPreparation, SpiredStructuralPreparationLimits,
};

const PRIME_A: u64 = 1_000_000_007;
const PRIME_B: u64 = 1_000_000_021;

fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
}

fn k1_case(artifact: &ClosedArtifact, completed: &CompletedIbpSourceRows) -> SpiredExecutionCase {
    let target = IntegralShift::try_new([1]).unwrap();
    let structural_shifts = completed.relations()[0]
        .terms()
        .keys()
        .map(|shift| shift.values())
        .collect::<Vec<_>>();
    let domain = SectorMonotoneDomain::try_maximal_for_rule(
        Mask::try_new([true]).unwrap(),
        target.values(),
        &structural_shifts,
    )
    .unwrap();
    let stratum = DecoratedStratum::try_guard_blind(
        artifact.family_fingerprint(),
        artifact.context_fingerprint(),
        domain,
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let owners = ImmutableOwnerSnapshot::try_empty(
        artifact.family_fingerprint(),
        artifact.context_fingerprint(),
        1,
        StratumRegistryLimits::default(),
    )
    .unwrap();
    SpiredExecutionCase::try_new(
        SpiredCase::CoordinateFace(SpiredCoordinateFace::new(stratum)),
        target,
        OrderingPolicy::default(),
        owners,
    )
    .unwrap()
}

fn k3_case(artifact: &Arc<ClosedArtifact>, cell_ordinal: usize) -> SpiredExecutionCase {
    let cell = &artifact.rule_cells()[cell_ordinal];
    let stratum = DecoratedStratum::try_guard_blind(
        artifact.family_fingerprint(),
        artifact.context_fingerprint(),
        cell.application_domain().clone(),
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let owners = ImmutableOwnerSnapshot::try_from_closed_artifact(
        Arc::clone(artifact),
        StratumRegistryLimits::default(),
    )
    .unwrap();
    SpiredExecutionCase::try_new(
        SpiredCase::CoordinateFace(SpiredCoordinateFace::new(stratum)),
        IntegralShift::try_new(cell.rule().pivot().values().iter().copied()).unwrap(),
        artifact.ordering(),
        owners,
    )
    .unwrap()
}

fn probe(modulus: u64, arity: usize, base_count: usize) -> CampaignModularProbe {
    CampaignModularProbe::try_new(
        modulus,
        vec![37; base_count],
        vec![2; arity],
        CampaignLimits::default(),
    )
    .unwrap()
}

fn zero_requests(source_count: usize, arity: usize) -> Vec<TranslatedSourceRequest> {
    (0..source_count)
        .map(|source_ordinal| {
            TranslatedSourceRequest::new(
                source_ordinal,
                IntegralShift::try_new(vec![0; arity]).unwrap(),
            )
        })
        .collect()
}

#[test]
fn k1_shared_plan_matches_probe_local_classification_for_two_probes() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let case = k1_case(&artifact, &completed);
    let requests = zero_requests(completed.source_row_count(), 1);
    let mut preparation = SpiredStructuralPreparation::try_new(
        generator.context(),
        &completed,
        &case,
        SpiredStructuralPreparationLimits::default(),
    )
    .unwrap();
    let plan = preparation.try_prepare_requests(&requests).unwrap();
    assert_eq!(preparation.census().exact_sources().source_rows(), 1);
    assert_eq!(preparation.census().prepared_rows(), 1);

    for modulus in [PRIME_B, PRIME_A] {
        let raw_probe = probe(
            modulus,
            1,
            generator.context().base().parameter_names().len(),
        );
        let mut prepared = SpiredPreparedStreamingDiscovery::try_new(
            &preparation,
            &raw_probe,
            SpiredModularLimits::default(),
        )
        .unwrap();
        let mut local = SpiredStreamingDiscovery::try_new(
            generator.context(),
            &completed,
            &case,
            &raw_probe,
            SpiredStreamingLimits::default(),
        )
        .unwrap();
        let prepared_hit = prepared.try_consume_chunk(&plan).unwrap().unwrap();
        let local_hit = local.try_consume_chunk(&requests).unwrap().unwrap();
        assert_eq!(prepared_hit.rows_consumed(), local_hit.rows_consumed());
        assert_eq!(prepared_hit.forbidden_rank(), local_hit.forbidden_rank());
        assert_eq!(prepared_hit.augmented_rank(), local_hit.augmented_rank());
        assert_eq!(prepared_hit.support(), local_hit.support());
        assert_eq!(prepared.retained_structural_coordinate_cells(), 0);
    }
    // Creating and consuming two probes does not repeat or mutate exact
    // source/role work in the shared census.
    assert_eq!(preparation.census().exact_sources().source_rows(), 1);
    assert_eq!(preparation.census().source_term_inspections(), 2);
}

#[test]
fn k3_plan_is_deterministic_and_matches_legacy_streaming() {
    let artifact = Arc::new(derive_two_loop_unit_mass_sunset().unwrap());
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    assert_eq!(completed.source_row_count(), 4);
    let case = k3_case(&artifact, 0);
    let requests = zero_requests(completed.source_row_count(), 3);
    let limits = SpiredStructuralPreparationLimits::default();
    let mut first =
        SpiredStructuralPreparation::try_new(generator.context(), &completed, &case, limits)
            .unwrap();
    let mut second =
        SpiredStructuralPreparation::try_new(generator.context(), &completed, &case, limits)
            .unwrap();
    let first_plan = first.try_prepare_requests(&requests).unwrap();
    let second_plan = second.try_prepare_requests(&requests).unwrap();
    assert!(first_plan.same_structural_plan_as(&second_plan));
    assert_eq!(first.census(), second.census());

    let raw_probe = probe(
        PRIME_A,
        3,
        generator.context().base().parameter_names().len(),
    );
    let mut prepared = SpiredPreparedStreamingDiscovery::try_new(
        &first,
        &raw_probe,
        SpiredModularLimits::default(),
    )
    .unwrap();
    let mut local = SpiredStreamingDiscovery::try_new(
        generator.context(),
        &completed,
        &case,
        &raw_probe,
        SpiredStreamingLimits::default(),
    )
    .unwrap();
    let prepared_hit = prepared.try_consume_chunk(&first_plan).unwrap().unwrap();
    let mut local_hit = None;
    for request in &requests {
        if let Some(hit) = local.try_consume_request(request).unwrap() {
            local_hit = Some(hit);
            break;
        }
    }
    let local_hit = local_hit.unwrap();
    assert_eq!(prepared_hit.rows_consumed(), local_hit.rows_consumed());
    assert_eq!(prepared_hit.forbidden_rank(), local_hit.forbidden_rank());
    assert_eq!(prepared_hit.augmented_rank(), local_hit.augmented_rank());
    assert_eq!(prepared_hit.support(), local_hit.support());

    let mut previous_frontier = 0usize;
    for row in first_plan.rows() {
        assert!(row.forbidden_frontier() >= previous_frontier);
        for id in previous_frontier..row.forbidden_frontier() {
            assert!(
                row.term_roles()
                    .contains(&SpiredPreparedTermRole::Forbidden(
                        u32::try_from(id).unwrap()
                    ))
            );
        }
        previous_frontier = row.forbidden_frontier();
    }
    assert_eq!(
        previous_frontier,
        first.census().forbidden_columns(),
        "every stable ID must first appear in its scheduler-chronology row"
    );
}

#[test]
fn shared_work_and_memory_census_is_not_multiplied_by_probe_count() {
    let artifact = Arc::new(derive_two_loop_unit_mass_sunset().unwrap());
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let case = k3_case(&artifact, 0);
    let requests = zero_requests(completed.source_row_count(), 3);
    let mut preparation = SpiredStructuralPreparation::try_new(
        generator.context(),
        &completed,
        &case,
        SpiredStructuralPreparationLimits::default(),
    )
    .unwrap();
    let plan = preparation.try_prepare_requests(&requests).unwrap();
    let baseline = preparation.census();
    assert_eq!(baseline.exact_sources().source_rows(), 4);
    assert_eq!(baseline.prepared_rows(), 4);
    assert_eq!(baseline.request_inspections(), 4);
    assert_eq!(
        baseline.source_term_inspections(),
        baseline.emitted_term_roles()
    );
    assert!(baseline.unique_shifts() > 0);
    assert!(baseline.unique_shift_coordinate_cells() >= baseline.unique_shifts() * 3);
    assert!(baseline.emitted_plan_payload_bytes_lower_bound() > 0);
    assert!(baseline.registry_payload_bytes_lower_bound() > 0);

    for modulus in [PRIME_A, PRIME_B] {
        let raw_probe = probe(
            modulus,
            3,
            generator.context().base().parameter_names().len(),
        );
        let mut discovery = SpiredPreparedStreamingDiscovery::try_new(
            &preparation,
            &raw_probe,
            SpiredModularLimits::default(),
        )
        .unwrap();
        assert_eq!(discovery.retained_structural_coordinate_cells(), 0);
        assert!(discovery.try_consume_chunk(&plan).unwrap().is_some());
    }
    assert_eq!(preparation.census(), baseline);
}

#[test]
fn a_plan_from_another_exact_preparation_scope_fails_closed() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let case = k1_case(&artifact, &completed);
    let requests = zero_requests(1, 1);
    let limits = SpiredStructuralPreparationLimits::default();
    let first =
        SpiredStructuralPreparation::try_new(generator.context(), &completed, &case, limits)
            .unwrap();
    let mut second =
        SpiredStructuralPreparation::try_new(generator.context(), &completed, &case, limits)
            .unwrap();
    let foreign_plan = second.try_prepare_requests(&requests).unwrap();
    let raw_probe = probe(
        PRIME_A,
        1,
        generator.context().base().parameter_names().len(),
    );
    let mut discovery = SpiredPreparedStreamingDiscovery::try_new(
        &first,
        &raw_probe,
        SpiredModularLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        discovery.try_consume_chunk(&foreign_plan),
        Err(super::super::SpiredStreamingError::PreparedPlanScopeMismatch)
    ));
    assert!(discovery.is_poisoned());
}

#[test]
fn excluded_prepared_row_advances_only_plan_chronology() {
    let artifact = Arc::new(derive_two_loop_unit_mass_sunset().unwrap());
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let case = k3_case(&artifact, 0);
    let requests = zero_requests(completed.source_row_count(), 3);
    let mut preparation = SpiredStructuralPreparation::try_new(
        generator.context(),
        &completed,
        &case,
        SpiredStructuralPreparationLimits::default(),
    )
    .unwrap();
    let excluded = preparation.try_prepare_requests(&requests[..1]).unwrap();
    let admitted = preparation.try_prepare_requests(&requests[1..2]).unwrap();
    assert!(
        excluded.rows()[0]
            .term_roles()
            .iter()
            .any(|role| matches!(role, SpiredPreparedTermRole::Forbidden(_)))
    );

    let raw_probe = probe(
        PRIME_A,
        3,
        generator.context().base().parameter_names().len(),
    );
    let mut discovery = SpiredPreparedStreamingDiscovery::try_new(
        &preparation,
        &raw_probe,
        SpiredModularLimits::default(),
    )
    .unwrap();
    discovery.try_skip_chunk(&excluded).unwrap();
    assert_eq!(discovery.rows_consumed(), 0);
    assert_eq!(discovery.registered_forbidden_column_count(), 0);

    let _ = discovery.try_consume_chunk(&admitted).unwrap();
    assert_eq!(discovery.rows_consumed(), 1);
}

#[test]
fn preparation_resource_limit_fails_before_structural_classification() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let case = k1_case(&artifact, &completed);
    let mut limits = SpiredStructuralPreparationLimits::default();
    limits.max_prepared_rows = 0;
    let mut preparation =
        SpiredStructuralPreparation::try_new(generator.context(), &completed, &case, limits)
            .unwrap();
    assert!(matches!(
        preparation.try_prepare_requests(&zero_requests(1, 1)),
        Err(super::SpiredStructuralPreparationError::ResourceLimit {
            resource: "prepared structural rows",
            requested: 1,
            limit: 0,
        })
    ));
    assert_eq!(preparation.census().unique_shifts(), 0);
    assert!(preparation.is_poisoned());
}

#[test]
fn prepared_post_hit_continuation_preserves_original_row_chronology() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let case = k1_case(&artifact, &completed);
    let repeated = zero_requests(1, 1).into_iter().next().unwrap();
    let requests = vec![repeated.clone(), repeated.clone(), repeated];
    let mut preparation = SpiredStructuralPreparation::try_new(
        generator.context(),
        &completed,
        &case,
        SpiredStructuralPreparationLimits::default(),
    )
    .unwrap();
    let plan = preparation.try_prepare_requests(&requests).unwrap();
    let raw_probe = probe(
        PRIME_A,
        1,
        generator.context().base().parameter_names().len(),
    );
    let mut discovery = SpiredPreparedStreamingDiscovery::try_new(
        &preparation,
        &raw_probe,
        SpiredModularLimits::default(),
    )
    .unwrap();
    let scope = preparation.scope();
    assert!(matches!(
        discovery
            .try_consume_prepared_row_continuing(&scope, &plan.rows()[0])
            .unwrap(),
        SpiredModularStreamOutcome::FirstHit(_)
    ));
    let candidate = match discovery
        .try_consume_prepared_row_continuing(&scope, &plan.rows()[1])
        .unwrap()
    {
        SpiredModularStreamOutcome::PostHitCandidate(candidate) => candidate,
        other => panic!("expected repeated prepared row to nominate a candidate, got {other:?}"),
    };
    assert_eq!(candidate.root_source(), &requests[1]);
    assert_eq!(discovery.rows_consumed(), 2);

    let mut with_post_hit_skip = SpiredPreparedStreamingDiscovery::try_new(
        &preparation,
        &raw_probe,
        SpiredModularLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        with_post_hit_skip
            .try_consume_prepared_row_continuing(&scope, &plan.rows()[0])
            .unwrap(),
        SpiredModularStreamOutcome::FirstHit(_)
    ));
    with_post_hit_skip
        .try_skip_prepared_row_view_continuing(&scope, plan.rows()[1].view())
        .unwrap();
    assert!(matches!(
        with_post_hit_skip
            .try_consume_prepared_row_continuing(&scope, &plan.rows()[2])
            .unwrap(),
        SpiredModularStreamOutcome::PostHitCandidate(_)
    ));
    assert_eq!(with_post_hit_skip.rows_consumed(), 2);

    let mut out_of_order = SpiredPreparedStreamingDiscovery::try_new(
        &preparation,
        &raw_probe,
        SpiredModularLimits::default(),
    )
    .unwrap();
    out_of_order
        .try_consume_prepared_row_continuing(&scope, &plan.rows()[0])
        .unwrap();
    assert!(matches!(
        out_of_order.try_consume_prepared_row_continuing(&scope, &plan.rows()[2]),
        Err(super::super::SpiredStreamingError::PreparedPlanChronology {
            expected_row: 1,
            actual_row: 2,
        })
    ));
    assert!(out_of_order.is_poisoned());
    assert_eq!(out_of_order.rows_consumed(), 1);
}
