use crate::algebra::CoefficientContext;
use crate::family::{AffineDenominator, IntegralFamily};
use crate::foundry::artifact::derive_one_loop_unit_mass_tadpole;
use crate::foundry::completion::frame::modular::ModularTargetQuery;
use crate::foundry::completion::stratum::{
    DecoratedStratum, ImmutableOwnerSnapshot, StratumRegistryLimits,
};
use crate::identity::{
    CompletedIbpSourceRows, IntegralShift, ParametricIbpGenerator, TranslatedSourceLimits,
    TranslatedSourceRequest,
};
use crate::sector::{InteriorBounds, Mask, OrderingPolicy, SectorMonotoneDomain};

use super::{
    AccumulatedSourceRequests, CampaignError, CampaignLimits, CampaignModularProbe,
    CampaignRequestMerge, CampaignResourceStage, FreshTaskEpoch,
};
use crate::foundry::completion::source_discovery::{
    OrdinarySourceIncidenceIndex, SourceDiscoveryLimits,
};

const PRIME: u64 = 1_000_000_007;

fn request(source: usize, offset: i64) -> TranslatedSourceRequest {
    TranslatedSourceRequest::new(source, IntegralShift::try_new([offset]).unwrap())
}

fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
}

fn complete_external(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_external_ibp_sources().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
}

fn one_loop_one_external(name: &str) -> IntegralFamily {
    let context = CoefficientContext::new(["d", "s"]);
    IntegralFamily::new(
        name,
        vec!["k".to_owned()],
        vec!["p".to_owned()],
        context.clone(),
        context.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(context.integer(-1), vec![context.one(), context.zero()]),
            AffineDenominator::new(context.zero(), vec![context.zero(), context.one()]),
        ],
        vec![vec![context.parameter("s").unwrap()]],
        vec![context.zero(), context.zero()],
    )
    .unwrap()
}

fn fixed_tadpole_inputs(
    family: &crate::foundry::artifact::ClosedArtifact,
    pivot: i64,
    physical_shifts: &[Vec<i64>],
) -> (DecoratedStratum, ImmutableOwnerSnapshot) {
    let domain = SectorMonotoneDomain::try_maximal_for_rule(
        Mask::try_new([true]).unwrap(),
        &[pivot],
        physical_shifts,
    )
    .unwrap();
    let limits = StratumRegistryLimits::default();
    let stratum = DecoratedStratum::try_guard_blind(
        family.family_fingerprint(),
        family.context_fingerprint(),
        domain,
        limits,
    )
    .unwrap();
    let owners = ImmutableOwnerSnapshot::try_empty(
        family.family_fingerprint(),
        family.context_fingerprint(),
        1,
        limits,
    )
    .unwrap();
    (stratum, owners)
}

#[test]
fn request_accumulation_and_merge_are_canonical_and_exactly_censused() {
    let limits = CampaignLimits::default();
    let initial = AccumulatedSourceRequests::try_new(
        1,
        [request(1, 1), request(0, 0), request(1, 1)],
        limits,
    )
    .unwrap();
    assert_eq!(initial.requests(), &[request(0, 0), request(1, 1)]);

    let forward = initial
        .try_merge_candidates(
            [request(1, 1), request(0, -1), request(2, 2), request(2, 2)],
            limits,
        )
        .unwrap();
    let reversed = initial
        .try_merge_candidates(
            [request(2, 2), request(2, 2), request(0, -1), request(1, 1)],
            limits,
        )
        .unwrap();
    assert_eq!(forward, reversed);
    let CampaignRequestMerge::Augmented {
        requests,
        telemetry,
    } = forward
    else {
        panic!("two novel requests must augment the immutable state")
    };
    assert_eq!(
        requests.requests(),
        &[request(0, -1), request(0, 0), request(1, 1), request(2, 2)]
    );
    assert_eq!(telemetry.submitted_candidates(), 4);
    assert_eq!(telemetry.canonical_candidates(), 3);
    assert_eq!(telemetry.duplicate_candidates(), 1);
    assert_eq!(telemetry.already_accumulated(), 1);
    assert_eq!(telemetry.added_requests(), 2);
    assert_eq!(telemetry.merged_request_count(), 4);
    assert_eq!(telemetry.merge_comparisons(), 3);

    let unchanged = initial
        .try_merge_candidates([request(1, 1), request(0, 0), request(0, 0)], limits)
        .unwrap();
    let CampaignRequestMerge::CandidateBatchExhausted(exhaustion) = unchanged else {
        panic!("an already accumulated finite candidate batch must remain unchanged")
    };
    assert_eq!(exhaustion.merge().submitted_candidates(), 3);
    assert_eq!(exhaustion.merge().canonical_candidates(), 2);
    assert_eq!(exhaustion.merge().already_accumulated(), 2);
    assert_eq!(exhaustion.merge().added_requests(), 0);
    assert_eq!(exhaustion.merge().merged_request_count(), initial.len());
    assert_eq!(
        CampaignRequestMerge::CandidateBatchExhausted(exhaustion).telemetry(),
        exhaustion.merge()
    );
}

#[test]
fn request_and_probe_resource_boundaries_return_typed_budget_telemetry() {
    let defaults = CampaignLimits::default();
    let mut limits = defaults;
    limits.max_submitted_requests = 1;
    let error =
        AccumulatedSourceRequests::try_new(1, [request(0, 0), request(0, 1)], limits).unwrap_err();
    let budget = error.budget_exhaustion().unwrap();
    assert_eq!(budget.stage(), CampaignResourceStage::RequestAccumulation);
    assert_eq!(budget.resource(), "campaign submitted source requests");
    assert_eq!(budget.requested(), 2);
    assert_eq!(budget.limit(), 1);

    let initial = AccumulatedSourceRequests::try_new(1, [request(0, 0)], defaults).unwrap();
    let mut comparison_limit = defaults;
    comparison_limit.max_merge_comparisons = 0;
    let error = initial
        .try_merge_candidates([request(0, 1)], comparison_limit)
        .unwrap_err();
    assert_eq!(
        error.budget_exhaustion().unwrap().resource(),
        "campaign stable request merge comparisons"
    );

    let exact_probe = CampaignModularProbe::try_new(PRIME, [37], [2], defaults).unwrap();
    assert_eq!(exact_probe.modulus(), PRIME);
    assert_eq!(exact_probe.base_parameters(), &[37]);
    assert_eq!(exact_probe.chart_coordinates(), &[2]);
    let mut probe_limit = defaults;
    probe_limit.max_retained_probe_coordinates = 1;
    let error = CampaignModularProbe::try_new(PRIME, [37], [2], probe_limit).unwrap_err();
    assert_eq!(
        error.budget_exhaustion().unwrap().resource(),
        "campaign retained raw probe coordinates"
    );
}

#[test]
fn fresh_single_row_epoch_returns_plan_bound_checked_obstruction() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = CampaignLimits::default();
    let requests = AccumulatedSourceRequests::try_new(1, [request(0, 0)], limits).unwrap();
    let target = IntegralShift::try_new([0]).unwrap();
    let (stratum, owners) = fixed_tadpole_inputs(&artifact, 0, &[vec![0], vec![1]]);
    let epoch = FreshTaskEpoch::try_new(
        0,
        &generator,
        &completed,
        requests,
        target,
        stratum,
        owners,
        OrderingPolicy::default(),
        limits,
    )
    .unwrap();
    assert_eq!(epoch.plan().row_count(), 1);
    assert_eq!(epoch.requests().len(), 1);
    assert_eq!(epoch.plan().columns().len(), 2);
    assert_eq!(epoch.target_shift().values(), &[0]);
    assert_eq!(epoch.plan().columns()[epoch.target_column()].values(), &[0]);

    let probe = CampaignModularProbe::try_new(PRIME, [37], [2], limits).unwrap();
    let evidence = epoch
        .try_query(generator.context(), &probe, limits)
        .unwrap();
    let obstruction = evidence
        .obstruction()
        .expect("one row cannot isolate the easier target from its harder partner");
    assert!(matches!(
        evidence.query(),
        ModularTargetQuery::NoHitWithObstruction(_)
    ));
    assert!(std::ptr::eq(obstruction.plan(), epoch.plan()));
    assert!(std::ptr::eq(evidence.sampled().plan(), epoch.plan()));
    assert!(std::sync::Arc::ptr_eq(
        evidence.sampled().sample_fingerprint(),
        obstruction.sample_fingerprint()
    ));
    assert!(std::ptr::eq(evidence.partition().frame(), epoch.plan()));
    assert_eq!(obstruction.target_physical_column(), epoch.target_column());
    assert_eq!(evidence.probe(), &probe);
    assert_eq!(evidence.telemetry().allowed_columns(), 0);
    assert_eq!(evidence.telemetry().forbidden_columns(), 1);
    assert_eq!(evidence.telemetry().forbidden_rank(), 1);
    assert_eq!(evidence.telemetry().augmented_rank(), 1);
}

#[test]
fn augmented_epochs_rebuild_all_plan_local_ordinals_and_can_hit() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = CampaignLimits::default();
    let first_requests = AccumulatedSourceRequests::try_new(1, [request(0, 0)], limits).unwrap();
    let CampaignRequestMerge::Augmented {
        requests: second_requests,
        ..
    } = first_requests
        .try_merge_candidates([request(0, 1)], limits)
        .unwrap()
    else {
        panic!("the translated second source row must be novel")
    };
    let augmented = first_requests
        .try_merge_candidates([request(0, 1)], limits)
        .unwrap();
    assert_eq!(augmented.augmented_requests(), Some(&second_requests));
    let (stratum, owners) = fixed_tadpole_inputs(&artifact, 1, &[vec![0], vec![1], vec![2]]);
    let target = IntegralShift::try_new([1]).unwrap();
    let first = FreshTaskEpoch::try_new(
        1,
        &generator,
        &completed,
        second_requests.clone(),
        target.clone(),
        stratum.clone(),
        owners.clone(),
        OrderingPolicy::default(),
        limits,
    )
    .unwrap();
    let repeated = FreshTaskEpoch::try_new(
        1,
        &generator,
        &completed,
        second_requests,
        target,
        stratum,
        owners,
        OrderingPolicy::default(),
        limits,
    )
    .unwrap();
    assert_eq!(first.plan(), repeated.plan());
    assert!(!first.plan().identity_owner().belongs_to(repeated.plan()));
    assert_eq!(first.telemetry(), repeated.telemetry());
    assert_eq!(first.telemetry().epoch_ordinal(), 1);
    assert_eq!(first.telemetry().request_count(), 2);
    assert_eq!(first.telemetry().physical_rows(), 2);
    assert_eq!(first.telemetry().physical_columns(), 3);
    assert_eq!(first.telemetry().physical_entries(), 4);
    assert_eq!(first.telemetry().target_column(), first.target_column());
    assert_eq!(first.fixed_ordering(), OrderingPolicy::default());
    assert_eq!(first.fixed_snapshot_id(), repeated.fixed_snapshot_id());

    let probe = CampaignModularProbe::try_new(PRIME, [37], [2], limits).unwrap();
    let first_evidence = first
        .try_query(generator.context(), &probe, limits)
        .unwrap();
    let repeated_evidence = repeated
        .try_query(generator.context(), &probe, limits)
        .unwrap();
    let ModularTargetQuery::Hit(first_hit) = first_evidence.query() else {
        panic!("two tadpole translations must isolate the middle target")
    };
    let ModularTargetQuery::Hit(repeated_hit) = repeated_evidence.query() else {
        panic!("the repeated fresh plan must produce the same sampled hit")
    };
    assert!(std::ptr::eq(first_hit.plan(), first.plan()));
    assert!(std::ptr::eq(repeated_hit.plan(), repeated.plan()));
    assert!(!std::ptr::eq(first_hit.plan(), repeated_hit.plan()));
    assert_eq!(first_evidence.telemetry(), repeated_evidence.telemetry());
    assert_eq!(first_evidence.telemetry().allowed_columns(), 1);
    assert_eq!(first_evidence.telemetry().forbidden_columns(), 1);
    assert_eq!(first_evidence.telemetry().forbidden_rank(), 1);
    assert_eq!(first_evidence.telemetry().augmented_rank(), 2);
}

#[test]
fn checked_obstruction_residuals_merge_into_a_fresh_executable_epoch() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = CampaignLimits::default();
    let source_limits = SourceDiscoveryLimits::default();
    let zero_sources = generator
        .translate_completed_source_rows(
            &completed,
            [IntegralShift::try_new([0]).unwrap()],
            TranslatedSourceLimits::default(),
        )
        .unwrap();
    let incidence = OrdinarySourceIncidenceIndex::try_new(&zero_sources, source_limits).unwrap();
    let requests = AccumulatedSourceRequests::try_new(1, [request(0, 0)], limits).unwrap();
    let domain = SectorMonotoneDomain::try_new_for_rule(
        Mask::try_new([true]).unwrap(),
        [InteriorBounds::new(10, 100)],
        &[0],
        &[vec![-1], vec![0], vec![1], vec![2]],
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
    let first = FreshTaskEpoch::try_new(
        0,
        &generator,
        &completed,
        requests,
        IntegralShift::try_new([0]).unwrap(),
        stratum.clone(),
        owners.clone(),
        OrderingPolicy::default(),
        limits,
    )
    .unwrap();
    let probe = CampaignModularProbe::try_new(PRIME, [37], [20], limits).unwrap();
    let first_evidence = first
        .try_query(generator.context(), &probe, limits)
        .unwrap();
    let obstruction = first_evidence
        .obstruction()
        .expect("the initial one-row task must expose its checked right obstruction");
    let nominations = incidence
        .try_nominate_obstruction(obstruction, source_limits)
        .unwrap();
    let residuals = incidence
        .try_retain_nonzero_residuals(
            &generator,
            &completed,
            &nominations,
            first_evidence.sampled(),
            obstruction,
            source_limits,
        )
        .unwrap();
    assert!(!residuals.requests().is_empty());
    let CampaignRequestMerge::Augmented {
        requests: accumulated,
        telemetry,
    } = first
        .requests()
        .try_merge_candidates(residuals.requests().iter().cloned(), limits)
        .unwrap()
    else {
        panic!("a nonzero residual request must create a fresh accumulated state")
    };
    assert_eq!(telemetry.added_requests(), residuals.requests().len());

    let second = FreshTaskEpoch::try_new(
        1,
        &generator,
        &completed,
        accumulated,
        IntegralShift::try_new([0]).unwrap(),
        stratum,
        owners,
        OrderingPolicy::default(),
        limits,
    )
    .unwrap();
    assert!(second.plan().row_count() > first.plan().row_count());
    assert!(!first.plan().identity_owner().belongs_to(second.plan()));
    let second_evidence = second
        .try_query(generator.context(), &probe, limits)
        .unwrap();
    assert!(std::ptr::eq(
        second_evidence.sampled().plan(),
        second.plan()
    ));
    assert_eq!(second_evidence.probe(), &probe);
}

#[test]
fn fixed_domain_is_never_widened_and_sample_membership_is_checked_before_query() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = CampaignLimits::default();
    let requests = AccumulatedSourceRequests::try_new(1, [request(0, 0)], limits).unwrap();
    let domain = SectorMonotoneDomain::try_new_for_rule(
        Mask::try_new([true]).unwrap(),
        [InteriorBounds::new(1, i64::MAX)],
        &[0],
        &[vec![0]],
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
    let epoch = FreshTaskEpoch::try_new(
        0,
        &generator,
        &completed,
        requests,
        IntegralShift::try_new([0]).unwrap(),
        stratum.clone(),
        owners,
        OrderingPolicy::default(),
        limits,
    )
    .unwrap();
    assert_eq!(epoch.fixed_stratum(), &stratum);
    let probe = CampaignModularProbe::try_new(PRIME, [37], [2], limits).unwrap();
    assert!(matches!(
        epoch.try_query(generator.context(), &probe, limits),
        Err(CampaignError::FixedStratumDoesNotCoverColumn { column: 1 })
    ));

    let (bounded, owners) = fixed_tadpole_inputs(&artifact, 0, &[vec![0], vec![1]]);
    let tightened = SectorMonotoneDomain::try_new_for_rule(
        Mask::try_new([true]).unwrap(),
        [InteriorBounds::new(1, 10)],
        &[0],
        &[vec![0], vec![1]],
    )
    .unwrap();
    let bounded = DecoratedStratum::try_guard_blind(
        bounded.family_fingerprint(),
        bounded.context_fingerprint(),
        tightened,
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let requests = AccumulatedSourceRequests::try_new(1, [request(0, 0)], limits).unwrap();
    let bounded_epoch = FreshTaskEpoch::try_new(
        1,
        &generator,
        &completed,
        requests,
        IntegralShift::try_new([0]).unwrap(),
        bounded,
        owners,
        OrderingPolicy::default(),
        limits,
    )
    .unwrap();
    let outside = CampaignModularProbe::try_new(PRIME, [37], [10], limits).unwrap();
    assert_eq!(
        bounded_epoch
            .try_query(generator.context(), &outside, limits)
            .unwrap_err(),
        CampaignError::SampleOutsideFixedStratum {
            position: 0,
            index: 11,
            lower: 1,
            upper: 10,
        }
    );
}

#[test]
fn campaign_rejects_external_only_source_chronology_before_frame_construction() {
    let family = one_loop_one_external("campaign-external-only");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_external(&generator);
    let limits = CampaignLimits::default();
    let requests = AccumulatedSourceRequests::try_new(
        2,
        [TranslatedSourceRequest::new(
            0,
            IntegralShift::try_new([0, 0]).unwrap(),
        )],
        limits,
    )
    .unwrap();
    let sector = Mask::try_new([true, false]).unwrap();
    let domain =
        SectorMonotoneDomain::try_maximal_for_rule(sector, &[0, 0], &[vec![0, 0]]).unwrap();
    let stratum = DecoratedStratum::try_guard_blind(
        "unused-family",
        generator.context().fingerprint(),
        domain,
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let owners = ImmutableOwnerSnapshot::try_empty(
        "unused-family",
        generator.context().fingerprint(),
        2,
        StratumRegistryLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        FreshTaskEpoch::try_new(
            0,
            &generator,
            &completed,
            requests,
            IntegralShift::try_new([0, 0]).unwrap(),
            stratum,
            owners,
            OrderingPolicy::default(),
            limits,
        ),
        Err(CampaignError::WrongSourceLayout {
            actual: "external-contraction IBP source"
        })
    ));
}

#[test]
fn campaign_rejects_a_foreign_fixed_task_scope_before_physical_assembly() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = CampaignLimits::default();
    let requests = AccumulatedSourceRequests::try_new(1, [request(0, 0)], limits).unwrap();
    let (correct, owners) = fixed_tadpole_inputs(&artifact, 0, &[vec![0], vec![1]]);
    let foreign = DecoratedStratum::try_guard_blind(
        "foreign-campaign-family",
        correct.context_fingerprint(),
        correct.domain().clone(),
        StratumRegistryLimits::default(),
    )
    .unwrap();

    assert_eq!(
        FreshTaskEpoch::try_new(
            0,
            &generator,
            &completed,
            requests,
            IntegralShift::try_new([0]).unwrap(),
            foreign,
            owners,
            OrderingPolicy::default(),
            limits,
        )
        .unwrap_err(),
        CampaignError::FixedTaskScopeMismatch {
            detail: "selected sources and decorated stratum belong to different families",
        }
    );
}

#[test]
fn nested_stage_limits_are_lifted_to_typed_campaign_budget_results() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let defaults = CampaignLimits::default();
    let requests =
        AccumulatedSourceRequests::try_new(1, [request(0, 0), request(0, 1)], defaults).unwrap();
    let (stratum, owners) = fixed_tadpole_inputs(&artifact, 1, &[vec![0], vec![1], vec![2]]);

    let mut translation_limit = defaults;
    translation_limit.translated_sources.max_translated_sources = 1;
    let error = FreshTaskEpoch::try_new(
        0,
        &generator,
        &completed,
        requests.clone(),
        IntegralShift::try_new([1]).unwrap(),
        stratum.clone(),
        owners.clone(),
        OrderingPolicy::default(),
        translation_limit,
    )
    .unwrap_err();
    assert_eq!(
        error.budget_exhaustion().unwrap().stage(),
        CampaignResourceStage::SelectedTranslation
    );

    let mut frame_limit = defaults;
    frame_limit.physical_frame.max_physical_columns = 2;
    let error = FreshTaskEpoch::try_new(
        0,
        &generator,
        &completed,
        requests.clone(),
        IntegralShift::try_new([1]).unwrap(),
        stratum.clone(),
        owners.clone(),
        OrderingPolicy::default(),
        frame_limit,
    )
    .unwrap_err();
    assert_eq!(
        error.budget_exhaustion().unwrap().stage(),
        CampaignResourceStage::PhysicalFrame
    );

    let epoch = FreshTaskEpoch::try_new(
        0,
        &generator,
        &completed,
        requests,
        IntegralShift::try_new([1]).unwrap(),
        stratum,
        owners,
        OrderingPolicy::default(),
        defaults,
    )
    .unwrap();
    let probe = CampaignModularProbe::try_new(PRIME, [37], [2], defaults).unwrap();
    let mut stratum_limit = defaults;
    stratum_limit.stratum.max_physical_columns = 2;
    let error = epoch
        .try_query(generator.context(), &probe, stratum_limit)
        .unwrap_err();
    assert_eq!(
        error.budget_exhaustion().unwrap().stage(),
        CampaignResourceStage::StratumPartition
    );

    let mut modular_limit = defaults;
    modular_limit.modular.max_matrix_rows = 1;
    let error = epoch
        .try_query(generator.context(), &probe, modular_limit)
        .unwrap_err();
    assert_eq!(
        error.budget_exhaustion().unwrap().stage(),
        CampaignResourceStage::ModularQuery
    );
}
