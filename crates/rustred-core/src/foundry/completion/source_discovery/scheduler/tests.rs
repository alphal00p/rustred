use crate::algebra::CoefficientContext;
use crate::family::{AffineDenominator, IntegralFamily};
use crate::foundry::artifact::{ClosedArtifact, derive_one_loop_unit_mass_tadpole};
use crate::foundry::completion::frame::modular::ModularKernelError;
use crate::foundry::completion::stratum::{
    DecoratedStratum, GuardBranch, GuardBranchIdentity, ImmutableOwnerSnapshot,
    MaximalStratumAnchor, StratumRegistryLimits,
};
use crate::identity::{
    CompletedIbpSourceRows, IntegralShift, ParametricIbpGenerator, TranslatedSourceRequest,
};
use crate::sector::{Mask, OrderingPolicy, SectorMonotoneDomain};

use super::super::{
    CampaignError, CampaignModularProbe, InitialParentSourceProposal, OrdinarySourceIncidenceIndex,
    SampledDeclaredModuleDualError, SourceDiscoveryLimits,
};
use super::{
    ProbeLocalBudgetCause, ProbeLocalBudgetScope, ProbeLocalIterationDisposition,
    ProbeLocalObstructionScheduler, ProbeLocalOutcome, ProbeLocalOutcomeKind, ProbeLocalRejection,
    ProbeLocalSchedulerError, ProbeLocalSchedulerLimits, ProbeLocalStage,
};

const PRIME: u64 = 1_000_000_007;

fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
}

fn probe(
    base: impl IntoIterator<Item = i64>,
    chart: impl IntoIterator<Item = u64>,
    limits: ProbeLocalSchedulerLimits,
) -> CampaignModularProbe {
    CampaignModularProbe::try_new(PRIME, base, chart, limits.campaign).unwrap()
}

fn maximal_anchor(stratum: DecoratedStratum) -> MaximalStratumAnchor {
    MaximalStratumAnchor::try_new(stratum, StratumRegistryLimits::default()).unwrap()
}

fn tadpole_inputs(
    artifact: &ClosedArtifact,
) -> (IntegralShift, DecoratedStratum, ImmutableOwnerSnapshot) {
    let target = IntegralShift::try_new([1]).unwrap();
    let domain = SectorMonotoneDomain::try_maximal_for_rule(
        Mask::try_new([true]).unwrap(),
        target.values(),
        &[vec![0], vec![1], vec![2]],
    )
    .unwrap();
    let registry = StratumRegistryLimits::default();
    let stratum = DecoratedStratum::try_guard_blind(
        artifact.family_fingerprint(),
        artifact.context_fingerprint(),
        domain,
        registry,
    )
    .unwrap();
    let owners = ImmutableOwnerSnapshot::try_empty(
        artifact.family_fingerprint(),
        artifact.context_fingerprint(),
        1,
        registry,
    )
    .unwrap();
    (target, stratum, owners)
}

fn tadpole_scheduler<'inputs, 'family>(
    generator: &'inputs ParametricIbpGenerator<'family>,
    completed: &'inputs CompletedIbpSourceRows,
    probes: impl IntoIterator<Item = CampaignModularProbe>,
    limits: ProbeLocalSchedulerLimits,
) -> ProbeLocalObstructionScheduler<'inputs, 'family> {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let (target, stratum, owners) = tadpole_inputs(&artifact);
    ProbeLocalObstructionScheduler::try_new(
        generator,
        completed,
        target,
        maximal_anchor(stratum),
        owners,
        OrderingPolicy::default(),
        probes,
        limits,
    )
    .unwrap()
}

fn tadpole_initial_parent_proposal(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    support: &[IntegralShift],
    limits: ProbeLocalSchedulerLimits,
) -> InitialParentSourceProposal {
    let zero_sources = generator
        .translate_completed_source_rows(
            completed,
            [IntegralShift::try_new([0]).unwrap()],
            limits.source_discovery.translation,
        )
        .unwrap();
    OrdinarySourceIncidenceIndex::try_new(&zero_sources, limits.source_discovery)
        .unwrap()
        .try_nominate_initial_parent_support(completed, support, limits.source_discovery)
        .unwrap()
}

fn tadpole_inputs_for_requests(
    artifact: &ClosedArtifact,
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    target: &IntegralShift,
    requests: &[TranslatedSourceRequest],
    limits: ProbeLocalSchedulerLimits,
) -> (MaximalStratumAnchor, ImmutableOwnerSnapshot) {
    let selected = generator
        .translate_selected_completed_source_rows(
            completed,
            requests.iter().cloned(),
            limits.source_discovery.translation,
        )
        .unwrap();
    let physical = selected
        .sources()
        .iter()
        .flat_map(|source| source.terms().keys())
        .map(|shift| shift.values().to_vec())
        .collect::<Vec<_>>();
    let domain = SectorMonotoneDomain::try_maximal_for_rule(
        Mask::try_new([true]).unwrap(),
        target.values(),
        &physical,
    )
    .unwrap();
    let registry = StratumRegistryLimits::default();
    let stratum = DecoratedStratum::try_guard_blind(
        artifact.family_fingerprint(),
        artifact.context_fingerprint(),
        domain,
        registry,
    )
    .unwrap();
    let owners = ImmutableOwnerSnapshot::try_empty(
        artifact.family_fingerprint(),
        artifact.context_fingerprint(),
        1,
        registry,
    )
    .unwrap();
    (maximal_anchor(stratum), owners)
}

fn one_loop_one_external(name: &str) -> IntegralFamily {
    let context = CoefficientContext::new(["d", "s"]);
    let inverse_mass = context.coefficient_fixture("-1/s");
    IntegralFamily::new(
        name,
        vec!["k".to_owned()],
        vec!["p".to_owned()],
        context.clone(),
        context.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(inverse_mass, vec![context.one(), context.zero()]),
            AffineDenominator::new(context.zero(), vec![context.zero(), context.one()]),
        ],
        vec![vec![context.parameter("s").unwrap()]],
        vec![context.zero(), context.zero()],
    )
    .unwrap()
}

fn external_inputs(
    family: &IntegralFamily,
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    limits: ProbeLocalSchedulerLimits,
) -> (IntegralShift, DecoratedStratum, ImmutableOwnerSnapshot) {
    let target = IntegralShift::try_new([0, 0]).unwrap();
    let zero_sources = generator
        .translate_completed_source_rows(
            completed,
            [IntegralShift::try_new([0, 0]).unwrap()],
            limits.source_discovery.translation,
        )
        .unwrap();
    let incidence =
        OrdinarySourceIncidenceIndex::try_new(&zero_sources, SourceDiscoveryLimits::default())
            .unwrap();
    let bootstrap = incidence
        .try_nominate_target_unit(&target, limits.source_discovery)
        .unwrap();
    let translated = generator
        .translate_selected_completed_source_rows(
            completed,
            bootstrap.requests().iter().cloned(),
            limits.source_discovery.translation,
        )
        .unwrap();
    let physical = translated
        .sources()
        .iter()
        .flat_map(|source| source.terms().keys())
        .map(|shift| shift.values().to_vec())
        .collect::<Vec<_>>();
    let domain = SectorMonotoneDomain::try_maximal_for_rule(
        Mask::try_new([true, true]).unwrap(),
        target.values(),
        &physical,
    )
    .unwrap();
    let registry = StratumRegistryLimits::default();
    let stratum = DecoratedStratum::try_guard_blind(
        family.fingerprint(),
        generator.context().fingerprint(),
        domain,
        registry,
    )
    .unwrap();
    let owners = ImmutableOwnerSnapshot::try_empty(
        family.fingerprint(),
        generator.context().fingerprint(),
        2,
        registry,
    )
    .unwrap();
    (target, stratum, owners)
}

fn request(source: usize, offset: i64) -> TranslatedSourceRequest {
    TranslatedSourceRequest::new(source, IntegralShift::try_new([offset]).unwrap())
}

#[test]
fn target_unit_bootstrap_hits_and_lifts_immediately() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ProbeLocalSchedulerLimits::default();
    let declared_probe = probe([37], [2], limits);
    let report = tadpole_scheduler(&generator, &completed, [declared_probe.clone()], limits)
        .run()
        .unwrap();

    assert_eq!(report.probes().len(), 1);
    let result = &report.probes()[0];
    assert_eq!(result.probe_ordinal(), 0);
    assert_eq!(result.probe(), &declared_probe);
    assert_eq!(result.outcome().kind(), ProbeLocalOutcomeKind::Replayed);
    assert_eq!(result.iterations().len(), 1);
    assert_eq!(result.iterations()[0].epoch_ordinal(), 0);
    assert_eq!(result.iterations()[0].request_count(), 2);
    assert_eq!(
        result.outcome().final_requests().unwrap(),
        &[request(0, 0), request(0, 1)]
    );
    assert_eq!(report.census().epochs(), 1);
    assert_eq!(report.census().exact_lift_attempts(), 1);
    assert_eq!(report.census().retained_iteration_records(), 1);
    assert!(result.outcome().replayed().is_some());
}

#[test]
fn initial_parent_requests_merge_deterministically_and_regenerate_in_each_fresh_probe_epoch() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ProbeLocalSchedulerLimits::default();
    let target = IntegralShift::try_new([1]).unwrap();
    let proposal = tadpole_initial_parent_proposal(
        &generator,
        &completed,
        &[IntegralShift::try_new([2]).unwrap()],
        limits,
    );
    let zero_sources = generator
        .translate_completed_source_rows(
            &completed,
            [IntegralShift::try_new([0]).unwrap()],
            limits.source_discovery.translation,
        )
        .unwrap();
    let incidence =
        OrdinarySourceIncidenceIndex::try_new(&zero_sources, limits.source_discovery).unwrap();
    let bootstrap = incidence
        .try_nominate_target_unit(&target, limits.source_discovery)
        .unwrap();

    let mut forward = bootstrap.requests().to_vec();
    forward.extend(proposal.requests().iter().cloned());
    forward.sort_unstable();
    forward.dedup();
    let mut reverse = proposal.requests().to_vec();
    reverse.extend(bootstrap.requests().iter().cloned());
    reverse.sort_unstable();
    reverse.dedup();
    assert_eq!(forward, reverse);
    assert!(forward.len() > bootstrap.requests().len());

    let (stratum, owners) =
        tadpole_inputs_for_requests(&artifact, &generator, &completed, &target, &forward, limits);
    let probes = [probe([37], [2], limits), probe([41], [3], limits)];
    let report = ProbeLocalObstructionScheduler::try_new_with_initial_parent_proposal(
        &generator,
        &completed,
        target,
        stratum,
        owners,
        OrderingPolicy::default(),
        proposal,
        probes,
        limits,
    )
    .unwrap()
    .run()
    .unwrap();

    assert_eq!(report.probes().len(), 2);
    for probe_report in report.probes() {
        assert_eq!(probe_report.iterations()[0].epoch_ordinal(), 0);
        assert_eq!(probe_report.iterations()[0].request_count(), forward.len());
        assert_eq!(
            probe_report.outcome().final_requests(),
            Some(forward.as_slice())
        );
        let epoch = probe_report.outcome().epoch().unwrap();
        let regenerated = epoch
            .plan()
            .source_instances()
            .iter()
            .map(|source| {
                TranslatedSourceRequest::new(
                    source.provenance().source_ordinal(),
                    source.provenance().offset().clone(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(regenerated, forward);
    }
    let first_epoch = report.probes()[0].outcome().epoch().unwrap();
    let second_epoch = report.probes()[1].outcome().epoch().unwrap();
    assert!(!std::ptr::eq(first_epoch.plan(), second_epoch.plan()));
    assert!(
        first_epoch
            .requests()
            .shares_storage_with(second_epoch.requests())
    );
}

#[test]
fn initial_parent_requests_are_rejected_once_before_probes_under_a_tight_request_cap() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let defaults = ProbeLocalSchedulerLimits::default();
    let target = IntegralShift::try_new([1]).unwrap();
    let proposal = tadpole_initial_parent_proposal(
        &generator,
        &completed,
        &[IntegralShift::try_new([2]).unwrap()],
        defaults,
    );
    let zero_sources = generator
        .translate_completed_source_rows(
            &completed,
            [IntegralShift::try_new([0]).unwrap()],
            defaults.source_discovery.translation,
        )
        .unwrap();
    let incidence =
        OrdinarySourceIncidenceIndex::try_new(&zero_sources, defaults.source_discovery).unwrap();
    let bootstrap = incidence
        .try_nominate_target_unit(&target, defaults.source_discovery)
        .unwrap();
    let mut expected = bootstrap.requests().to_vec();
    expected.extend(proposal.requests().iter().cloned());
    expected.sort_unstable();
    expected.dedup();
    assert!(expected.len() > 1);

    let (stratum, owners) = tadpole_inputs_for_requests(
        &artifact, &generator, &completed, &target, &expected, defaults,
    );
    let mut tight = defaults;
    tight.max_requests_per_probe = expected.len() - 1;
    let error = ProbeLocalObstructionScheduler::try_new_with_initial_parent_proposal(
        &generator,
        &completed,
        target,
        stratum,
        owners,
        OrderingPolicy::default(),
        proposal,
        [probe([37], [2], tight), probe([41], [3], tight)],
        tight,
    )
    .unwrap()
    .run()
    .unwrap_err();
    assert_eq!(
        error,
        ProbeLocalSchedulerError::ResourceLimit {
            resource: "probe-local requests per probe",
            requested: expected.len(),
            limit: expected.len() - 1,
        }
    );
}

#[test]
fn duplicate_integer_representatives_of_one_finite_field_probe_are_rejected() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ProbeLocalSchedulerLimits::default();
    let first = probe([37], [2], limits);
    let alias = probe([37 + PRIME as i64], [2 + PRIME], limits);
    let artifact_inputs = tadpole_inputs(&artifact);

    assert_eq!(
        ProbeLocalObstructionScheduler::try_new(
            &generator,
            &completed,
            artifact_inputs.0,
            maximal_anchor(artifact_inputs.1),
            artifact_inputs.2,
            OrderingPolicy::default(),
            [first, alias],
            limits,
        )
        .unwrap_err(),
        ProbeLocalSchedulerError::DuplicateProbe {
            first_ordinal: 0,
            duplicate_ordinal: 1,
        }
    );
}

#[test]
fn singular_probe_is_local_and_cannot_seed_its_good_sibling() {
    let family = one_loop_one_external("probe-local-singular-isolation");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ProbeLocalSchedulerLimits::default();
    let (target, stratum, owners) = external_inputs(&family, &generator, &completed, limits);
    let stratum = maximal_anchor(stratum);
    let singular = probe([37, 0], [0, 0], limits);
    let good = probe([37, 1], [0, 0], limits);
    let together = ProbeLocalObstructionScheduler::try_new(
        &generator,
        &completed,
        target.clone(),
        stratum.clone(),
        owners.clone(),
        OrderingPolicy::default(),
        [singular, good.clone()],
        limits,
    )
    .unwrap()
    .run()
    .unwrap();
    let alone = ProbeLocalObstructionScheduler::try_new(
        &generator,
        &completed,
        target,
        stratum,
        owners,
        OrderingPolicy::default(),
        [good],
        limits,
    )
    .unwrap()
    .run()
    .unwrap();

    assert_eq!(together.probes().len(), 2);
    assert_eq!(
        together.probes()[0].outcome().kind(),
        ProbeLocalOutcomeKind::Rejected
    );
    let ProbeLocalOutcome::Rejected {
        error: singular_error,
        ..
    } = together.probes()[0].outcome()
    else {
        unreachable!()
    };
    assert!(matches!(
        singular_error,
        ProbeLocalRejection::Campaign(CampaignError::Modular(
            ModularKernelError::CoefficientDenominatorZero { .. }
                | ModularKernelError::SourceConditionZero { .. }
        ))
    ));
    assert_ne!(
        together.probes()[0].iterations(),
        together.probes()[1].iterations()
    );
    assert_eq!(
        together.probes()[1].outcome().kind(),
        alone.probes()[0].outcome().kind()
    );
    assert_eq!(
        together.probes()[1].iterations(),
        alone.probes()[0].iterations()
    );
    assert_eq!(
        together.probes()[1].outcome().final_requests(),
        alone.probes()[0].outcome().final_requests()
    );
    assert!(together.probes()[1].iterations().len() >= 2);
    assert!(matches!(
        together.probes()[1].iterations(),
        [record, ..]
            if matches!(
                record.disposition(),
                ProbeLocalIterationDisposition::NoHitAugmented {
                    nonzero_residual_requests: 2,
                    added_requests: 2,
                    ..
                }
            )
    ));
}

#[test]
fn exhaustive_residual_census_can_feed_a_bounded_frontier_ranked_proposal_batch() {
    let family = one_loop_one_external("probe-local-bounded-residual-proposals");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let mut limits = ProbeLocalSchedulerLimits::default();
    limits.max_residual_proposals_per_iteration = 1;
    let (target, stratum, owners) = external_inputs(&family, &generator, &completed, limits);
    let report = ProbeLocalObstructionScheduler::try_new(
        &generator,
        &completed,
        target,
        maximal_anchor(stratum),
        owners,
        OrderingPolicy::default(),
        [probe([37, 1], [0, 0], limits)],
        limits,
    )
    .unwrap()
    .run()
    .unwrap();

    assert!(matches!(
        report.probes()[0].iterations(),
        [first, ..]
            if matches!(
                first.disposition(),
                ProbeLocalIterationDisposition::NoHitAugmented {
                    nonzero_residual_requests: 2,
                    added_requests: 1,
                    ..
                }
            )
    ));
    assert!(report.probes()[0].iterations().len() >= 3);
    assert_eq!(
        report.probes()[0].iterations()[1].request_count(),
        report.probes()[0].iterations()[0].request_count() + 1,
    );
    assert_eq!(
        report.probes()[0].iterations()[2].request_count(),
        report.probes()[0].iterations()[1].request_count() + 1,
        "an unselected cutter must remain eligible for a later fresh obstruction",
    );
    assert_eq!(
        report.probes()[0].outcome().kind(),
        ProbeLocalOutcomeKind::SampledDual,
        "a complete empty residual on the guard-blind fixture is admissible"
    );
    assert!(report.probes()[0].outcome().sampled_dual().is_some());
    assert!(report.probes()[0].outcome().rejection_summary().is_none());
}

#[test]
fn width_one_block_preserves_q0_frontier_order_and_reuses_primary_rows() {
    let family = one_loop_one_external("probe-local-width-one-cache-parity");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let mut limits = ProbeLocalSchedulerLimits::default();
    limits.max_residual_proposals_per_iteration = 1;
    limits.campaign.modular.max_obstruction_block_directions = 1;
    let (target, stratum, owners) = external_inputs(&family, &generator, &completed, limits);
    let report = ProbeLocalObstructionScheduler::try_new(
        &generator,
        &completed,
        target,
        maximal_anchor(stratum),
        owners,
        OrderingPolicy::default(),
        [probe([37, 1], [0, 0], limits)],
        limits,
    )
    .unwrap()
    .run()
    .unwrap();

    assert!(matches!(
        report.probes()[0].iterations(),
        [first, second, ..]
            if matches!(
                first.disposition(),
                ProbeLocalIterationDisposition::NoHitAugmented {
                    nonzero_residual_requests: 2,
                    added_requests: 1,
                    ..
                }
            )
                && second.request_count() == first.request_count() + 1
    ));
    let census = report.census();
    assert!(census.row_cache_hits() > 0);
    assert!(census.row_cache_physical_evaluations() < census.row_cache_logical_rows());
    assert!(census.row_cache_rows() <= census.row_cache_physical_evaluations());
    assert!(census.row_cache_value_cells() > 0);
}

#[test]
fn zero_residual_proposal_cap_is_rejected_at_task_admission() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let mut limits = ProbeLocalSchedulerLimits::default();
    limits.max_residual_proposals_per_iteration = 0;
    let (target, stratum, owners) = tadpole_inputs(&artifact);

    assert_eq!(
        ProbeLocalObstructionScheduler::try_new(
            &generator,
            &completed,
            target,
            maximal_anchor(stratum),
            owners,
            OrderingPolicy::default(),
            [probe([37], [2], limits)],
            limits,
        )
        .unwrap_err(),
        ProbeLocalSchedulerError::ResourceLimit {
            resource: "probe-local residual proposals per iteration",
            requested: 1,
            limit: 0,
        }
    );
}

#[test]
fn complete_empty_no_hit_census_returns_sampled_dual() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let mut limits = ProbeLocalSchedulerLimits::default();
    limits.source_discovery.max_residual_classifications = 0;
    let report = tadpole_scheduler(
        &generator,
        &completed,
        [probe([2], [PRIME - 1], limits)],
        limits,
    )
    .run()
    .unwrap();

    assert_eq!(
        report.probes()[0].outcome().kind(),
        ProbeLocalOutcomeKind::SampledDual,
        "unexpected degenerate no-hit outcome: {:#?}",
        report.probes()[0].outcome()
    );
    assert!(matches!(
        report.probes()[0].iterations(),
        [record]
            if matches!(
                record.disposition(),
                ProbeLocalIterationDisposition::NoHitEmptyResidual { .. }
            )
    ));
    assert!(report.probes()[0].outcome().sampled_dual().is_some());
    assert_eq!(report.census().residual_candidate_work(), 0);
    assert_eq!(report.census().residual_source_term_work(), 0);
    assert_eq!(report.census().prospective_classification_reservation(), 0);
}

#[test]
fn aggregate_residual_work_caps_are_explicit_inconclusive_stops() {
    let family = one_loop_one_external("probe-local-aggregate-residual-budgets");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);

    for (resource, configure) in [
        ("probe-local aggregate residual candidate work", 0usize),
        ("probe-local aggregate residual source-term work", 1usize),
        (
            "probe-local aggregate prospective classification work",
            2usize,
        ),
    ] {
        let mut limits = ProbeLocalSchedulerLimits::default();
        match configure {
            0 => limits.max_aggregate_residual_candidate_work = 0,
            1 => limits.max_aggregate_residual_source_term_work = 0,
            2 => limits.max_aggregate_prospective_classification_work = 0,
            _ => unreachable!(),
        }
        let (target, stratum, owners) = external_inputs(&family, &generator, &completed, limits);
        let report = ProbeLocalObstructionScheduler::try_new(
            &generator,
            &completed,
            target,
            maximal_anchor(stratum),
            owners,
            OrderingPolicy::default(),
            [probe([37, 1], [0, 0], limits)],
            limits,
        )
        .unwrap()
        .run()
        .unwrap();
        let ProbeLocalOutcome::BudgetStop { context, stop } = report.probes()[0].outcome() else {
            panic!(
                "aggregate residual cap must stop inconclusively: {:#?}",
                report.probes()[0].outcome(),
            )
        };
        assert_eq!(stop.stage(), ProbeLocalStage::ResidualEvaluation);
        assert_eq!(stop.cause().scope(), ProbeLocalBudgetScope::Aggregate);
        assert_eq!(stop.cause().resource(), resource);
        assert!(context.epoch().is_some());
        assert!(matches!(
            report.probes()[0].iterations(),
            [record]
                if record.disposition()
                    == ProbeLocalIterationDisposition::NoHitStopped {
                        stage: ProbeLocalStage::ResidualEvaluation,
                    }
        ));
    }
}

#[test]
fn every_union_nomination_reservation_cap_stops_before_union_materialization() {
    let family = one_loop_one_external("probe-local-union-nomination-preflight");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);

    for (resource, configured_cap) in [
        (
            "probe-local aggregate obstruction-block nomination raw-entry reservation",
            0usize,
        ),
        (
            "probe-local aggregate obstruction-block nomination raw-request reservation",
            1usize,
        ),
        (
            "probe-local aggregate obstruction-block nomination coordinate-cell reservation",
            2usize,
        ),
        (
            "probe-local aggregate obstruction-block nomination coefficient-cell reservation",
            3usize,
        ),
        (
            "probe-local aggregate obstruction-block nomination canonicalization-work reservation",
            4usize,
        ),
        (
            "probe-local aggregate obstruction-block nomination subset-comparison reservation",
            5usize,
        ),
    ] {
        let mut limits = ProbeLocalSchedulerLimits::default();
        match configured_cap {
            0 => {
                limits.max_aggregate_obstruction_block_nomination_raw_entry_reservation = 0;
            }
            1 => {
                limits.max_aggregate_obstruction_block_nomination_raw_request_reservation = 0;
            }
            2 => {
                limits.max_aggregate_obstruction_block_nomination_coordinate_cell_reservation = 0;
            }
            3 => {
                limits.max_aggregate_obstruction_block_nomination_coefficient_cell_reservation = 0;
            }
            4 => {
                limits
                    .max_aggregate_obstruction_block_nomination_canonicalization_work_reservation =
                    0;
            }
            5 => {
                limits.max_aggregate_obstruction_block_nomination_subset_comparison_reservation = 0;
            }
            _ => unreachable!(),
        }
        let (target, stratum, owners) = external_inputs(&family, &generator, &completed, limits);
        let report = ProbeLocalObstructionScheduler::try_new(
            &generator,
            &completed,
            target,
            maximal_anchor(stratum),
            owners,
            OrderingPolicy::default(),
            [probe([37, 1], [0, 0], limits)],
            limits,
        )
        .unwrap()
        .run()
        .unwrap();

        let ProbeLocalOutcome::BudgetStop { stop, .. } = report.probes()[0].outcome() else {
            panic!(
                "union nomination preflight must stop inconclusively: {:#?}",
                report.probes()[0].outcome(),
            )
        };
        assert_eq!(stop.stage(), ProbeLocalStage::ObstructionBlockNomination);
        assert_eq!(stop.cause().scope(), ProbeLocalBudgetScope::Aggregate);
        assert_eq!(stop.cause().resource(), resource);
        let census = report.census();
        assert_eq!(
            census.obstruction_block_nomination_raw_entry_reservation(),
            0
        );
        assert_eq!(
            census.obstruction_block_nomination_raw_request_reservation(),
            0
        );
        assert_eq!(
            census.obstruction_block_nomination_coordinate_cell_reservation(),
            0
        );
        assert_eq!(
            census.obstruction_block_nomination_coefficient_cell_reservation(),
            0
        );
        assert_eq!(
            census.obstruction_block_nomination_canonicalization_work_reservation(),
            0
        );
        assert_eq!(
            census.obstruction_block_nomination_subset_comparison_reservation(),
            0
        );
        assert_eq!(census.obstruction_block_candidate_work(), 0);
        assert_eq!(census.obstruction_block_source_term_work(), 0);
        assert_eq!(census.obstruction_block_signature_work(), 0);
        assert!(census.row_cache_physical_evaluations() > 0);
    }
}

#[test]
fn union_nomination_reservation_is_atomic_across_repeated_epochs() {
    let family = one_loop_one_external("probe-local-union-nomination-atomicity");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);

    let mut one_epoch_limits = ProbeLocalSchedulerLimits::default();
    one_epoch_limits.max_residual_proposals_per_iteration = 1;
    one_epoch_limits.max_iterations_per_probe = 1;
    let (target, stratum, owners) =
        external_inputs(&family, &generator, &completed, one_epoch_limits);
    let baseline = ProbeLocalObstructionScheduler::try_new(
        &generator,
        &completed,
        target.clone(),
        maximal_anchor(stratum.clone()),
        owners.clone(),
        OrderingPolicy::default(),
        [probe([37, 1], [0, 0], one_epoch_limits)],
        one_epoch_limits,
    )
    .unwrap()
    .run()
    .unwrap();
    assert!(matches!(
        baseline.probes()[0].iterations(),
        [first]
            if matches!(
                first.disposition(),
                ProbeLocalIterationDisposition::NoHitAugmented { added_requests: 1, .. }
            )
    ));
    let first = baseline.census();
    assert!(first.obstruction_block_nomination_raw_entry_reservation() > 0);

    let mut exact_limits = one_epoch_limits;
    exact_limits.max_aggregate_obstruction_block_nomination_raw_entry_reservation =
        first.obstruction_block_nomination_raw_entry_reservation();
    exact_limits.max_aggregate_obstruction_block_nomination_raw_request_reservation =
        first.obstruction_block_nomination_raw_request_reservation();
    exact_limits.max_aggregate_obstruction_block_nomination_coordinate_cell_reservation =
        first.obstruction_block_nomination_coordinate_cell_reservation();
    exact_limits.max_aggregate_obstruction_block_nomination_coefficient_cell_reservation =
        first.obstruction_block_nomination_coefficient_cell_reservation();
    exact_limits.max_aggregate_obstruction_block_nomination_canonicalization_work_reservation =
        first.obstruction_block_nomination_canonicalization_work_reservation();
    exact_limits.max_aggregate_obstruction_block_nomination_subset_comparison_reservation =
        first.obstruction_block_nomination_subset_comparison_reservation();

    let exact = ProbeLocalObstructionScheduler::try_new(
        &generator,
        &completed,
        target.clone(),
        maximal_anchor(stratum.clone()),
        owners.clone(),
        OrderingPolicy::default(),
        [probe([37, 1], [0, 0], exact_limits)],
        exact_limits,
    )
    .unwrap()
    .run()
    .unwrap();
    assert_eq!(
        exact.census(),
        first,
        "the exact first-epoch envelope must pass"
    );

    let mut repeated_limits = exact_limits;
    repeated_limits.max_iterations_per_probe = 4;
    let repeated = ProbeLocalObstructionScheduler::try_new(
        &generator,
        &completed,
        target,
        maximal_anchor(stratum),
        owners,
        OrderingPolicy::default(),
        [probe([37, 1], [0, 0], repeated_limits)],
        repeated_limits,
    )
    .unwrap()
    .run()
    .unwrap();
    assert!(matches!(
        repeated.probes()[0].iterations(),
        [first_record, second_record]
            if matches!(
                first_record.disposition(),
                ProbeLocalIterationDisposition::NoHitAugmented { added_requests: 1, .. }
            ) && second_record.disposition()
                == ProbeLocalIterationDisposition::NoHitStopped {
                    stage: ProbeLocalStage::ObstructionBlockNomination,
                }
    ));
    let ProbeLocalOutcome::BudgetStop { stop, .. } = repeated.probes()[0].outcome() else {
        panic!("the second union reservation must stop before materialization")
    };
    assert_eq!(stop.stage(), ProbeLocalStage::ObstructionBlockNomination);
    assert_eq!(stop.cause().scope(), ProbeLocalBudgetScope::Aggregate);
    let repeated_census = repeated.census();
    assert_eq!(
        repeated_census.obstruction_block_nomination_raw_entry_reservation(),
        first.obstruction_block_nomination_raw_entry_reservation()
    );
    assert_eq!(
        repeated_census.obstruction_block_nomination_raw_request_reservation(),
        first.obstruction_block_nomination_raw_request_reservation()
    );
    assert_eq!(
        repeated_census.obstruction_block_nomination_coordinate_cell_reservation(),
        first.obstruction_block_nomination_coordinate_cell_reservation()
    );
    assert_eq!(
        repeated_census.obstruction_block_nomination_coefficient_cell_reservation(),
        first.obstruction_block_nomination_coefficient_cell_reservation()
    );
    assert_eq!(
        repeated_census.obstruction_block_nomination_canonicalization_work_reservation(),
        first.obstruction_block_nomination_canonicalization_work_reservation()
    );
    assert_eq!(
        repeated_census.obstruction_block_nomination_subset_comparison_reservation(),
        first.obstruction_block_nomination_subset_comparison_reservation()
    );
    assert_eq!(
        repeated_census.obstruction_block_candidate_work(),
        first.obstruction_block_candidate_work()
    );
    assert_eq!(
        repeated_census.obstruction_block_source_term_work(),
        first.obstruction_block_source_term_work()
    );
    assert_eq!(
        repeated_census.obstruction_block_signature_work(),
        first.obstruction_block_signature_work()
    );
    assert_eq!(
        repeated_census.obstruction_block_selection_work(),
        first.obstruction_block_selection_work()
    );
}

#[test]
fn aggregate_logical_cache_cap_stops_before_primary_row_evaluation() {
    let family = one_loop_one_external("probe-local-aggregate-cache-preflight");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let mut limits = ProbeLocalSchedulerLimits::default();
    limits.max_aggregate_row_cache_logical_rows = 0;
    let (target, stratum, owners) = external_inputs(&family, &generator, &completed, limits);
    let report = ProbeLocalObstructionScheduler::try_new(
        &generator,
        &completed,
        target,
        maximal_anchor(stratum),
        owners,
        OrderingPolicy::default(),
        [probe([37, 1], [0, 0], limits)],
        limits,
    )
    .unwrap()
    .run()
    .unwrap();

    let ProbeLocalOutcome::BudgetStop { context, stop } = report.probes()[0].outcome() else {
        panic!(
            "logical cache preflight must stop inconclusively: {:#?}",
            report.probes()[0].outcome(),
        )
    };
    assert_eq!(stop.stage(), ProbeLocalStage::ResidualEvaluation);
    assert_eq!(stop.cause().scope(), ProbeLocalBudgetScope::Aggregate);
    assert_eq!(
        stop.cause().resource(),
        "probe-local aggregate row-cache logical rows"
    );
    assert!(context.epoch().is_some());
    assert_eq!(report.census().row_cache_physical_evaluations(), 0);
    assert_eq!(report.census().row_cache_rows(), 0);
}

#[test]
fn every_aggregate_actual_cache_cap_preflights_before_cache_mutation() {
    let family = one_loop_one_external("probe-local-aggregate-actual-cache-preflight");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);

    for (resource, configured_cap) in [
        ("probe-local aggregate row-cache rows", 0usize),
        ("probe-local aggregate row-cache value cells", 1usize),
        ("probe-local aggregate row-cache lookup comparisons", 2usize),
        (
            "probe-local aggregate row-cache physical evaluations",
            3usize,
        ),
        ("probe-local aggregate row-cache hits", 4usize),
        ("probe-local aggregate row-cache insertion moves", 5usize),
    ] {
        let mut limits = ProbeLocalSchedulerLimits::default();
        match configured_cap {
            0 => limits.max_aggregate_row_cache_rows = 0,
            1 => limits.max_aggregate_row_cache_value_cells = 0,
            2 => limits.max_aggregate_row_cache_lookup_comparisons = 0,
            3 => limits.max_aggregate_row_cache_physical_evaluations = 0,
            4 => limits.max_aggregate_row_cache_hits = 0,
            5 => limits.max_aggregate_row_cache_insertion_moves = 0,
            _ => unreachable!(),
        }
        let (target, stratum, owners) = external_inputs(&family, &generator, &completed, limits);
        let report = ProbeLocalObstructionScheduler::try_new(
            &generator,
            &completed,
            target,
            maximal_anchor(stratum),
            owners,
            OrderingPolicy::default(),
            [probe([37, 1], [0, 0], limits)],
            limits,
        )
        .unwrap()
        .run()
        .unwrap();

        let ProbeLocalOutcome::BudgetStop { stop, .. } = report.probes()[0].outcome() else {
            panic!(
                "actual cache preflight must stop inconclusively: {:#?}",
                report.probes()[0].outcome(),
            )
        };
        assert_eq!(stop.stage(), ProbeLocalStage::ResidualEvaluation);
        assert_eq!(stop.cause().scope(), ProbeLocalBudgetScope::Aggregate);
        assert_eq!(stop.cause().resource(), resource);
        assert!(report.census().residual_candidate_work() > 0);
        assert_eq!(report.census().row_cache_rows(), 0);
        assert_eq!(report.census().row_cache_value_cells(), 0);
        assert_eq!(report.census().row_cache_lookup_comparisons(), 0);
        assert_eq!(report.census().row_cache_physical_evaluations(), 0);
        assert_eq!(report.census().row_cache_hits(), 0);
        assert_eq!(report.census().row_cache_insertion_moves(), 0);
    }
}

#[test]
fn fallible_cache_batch_retains_exact_performed_work_in_terminal_census() {
    let family = one_loop_one_external("probe-local-fallible-cache-accounting");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let mut limits = ProbeLocalSchedulerLimits::default();
    limits.source_discovery.max_row_cache_physical_evaluations = 1;
    let (target, stratum, owners) = external_inputs(&family, &generator, &completed, limits);
    let report = ProbeLocalObstructionScheduler::try_new(
        &generator,
        &completed,
        target,
        maximal_anchor(stratum),
        owners,
        OrderingPolicy::default(),
        [probe([37, 1], [0, 0], limits)],
        limits,
    )
    .unwrap()
    .run()
    .unwrap();

    assert!(matches!(
        report.probes()[0].outcome(),
        ProbeLocalOutcome::BudgetStop { stop, .. }
            if stop.stage() == ProbeLocalStage::ResidualEvaluation
    ));
    let census = report.census();
    assert_eq!(census.row_cache_rows(), 1);
    assert!(census.row_cache_value_cells() > 0);
    assert_eq!(census.row_cache_physical_evaluations(), 1);
    assert!(census.row_cache_lookup_comparisons() > 0);
    assert_eq!(census.row_cache_hits(), 0);
}

#[test]
fn guarded_empty_no_hit_fails_closed_without_sample_bound_predicate_witness() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ProbeLocalSchedulerLimits::default();
    let (target, guard_blind, owners) = tadpole_inputs(&artifact);
    let guard = GuardBranchIdentity::try_new(
        "probe-local-opaque-guard",
        GuardBranch::NonZero,
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let guarded = DecoratedStratum::try_new(
        guard_blind.family_fingerprint(),
        guard_blind.context_fingerprint(),
        guard_blind.domain().clone(),
        [guard],
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let report = ProbeLocalObstructionScheduler::try_new(
        &generator,
        &completed,
        target,
        maximal_anchor(guarded),
        owners,
        OrderingPolicy::default(),
        [probe([2], [PRIME - 1], limits)],
        limits,
    )
    .unwrap()
    .run()
    .unwrap();

    let ProbeLocalOutcome::Rejected { stage, error, .. } = report.probes()[0].outcome() else {
        panic!("guarded sampled-dual path must reject")
    };
    assert_eq!(*stage, ProbeLocalStage::SampledDualAdmission);
    assert_eq!(
        error,
        &ProbeLocalRejection::SampledDual(
            SampledDeclaredModuleDualError::GuardedStratumRequiresSampleWitness { guard_count: 1 }
        )
    );
    assert!(report.probes()[0].outcome().sampled_dual().is_none());
}

#[test]
fn aggregate_cap_marks_the_remaining_declared_suffix_unexecuted() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let mut limits = ProbeLocalSchedulerLimits::default();
    limits.max_aggregate_epochs = 1;
    let report = tadpole_scheduler(
        &generator,
        &completed,
        [
            probe([37], [2], limits),
            probe([41], [3], limits),
            probe([43], [4], limits),
        ],
        limits,
    )
    .run()
    .unwrap();

    assert_eq!(
        report.probes()[0].outcome().kind(),
        ProbeLocalOutcomeKind::Replayed
    );
    let ProbeLocalOutcome::BudgetStop { context, stop } = report.probes()[1].outcome() else {
        panic!("aggregate suffix must be an explicit BudgetStop")
    };
    assert_eq!(stop.stage(), ProbeLocalStage::EpochAdmission);
    assert_eq!(stop.cause().scope(), ProbeLocalBudgetScope::Aggregate);
    assert!(context.requests().is_some());
    let ProbeLocalOutcome::BudgetStop { context, stop } = report.probes()[2].outcome() else {
        panic!("remaining aggregate suffix must be an explicit BudgetStop")
    };
    assert_eq!(stop.stage(), ProbeLocalStage::UnexecutedAggregateSuffix);
    assert!(context.requests().is_none());
    assert!(matches!(
        stop.cause(),
        ProbeLocalBudgetCause::UnexecutedAggregateSuffix {
            triggering_probe_ordinal: 1,
            resource: "probe-local aggregate fresh epochs",
        }
    ));
}

#[test]
fn repeated_declared_schedule_is_deterministic() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ProbeLocalSchedulerLimits::default();
    let run = || {
        tadpole_scheduler(
            &generator,
            &completed,
            [probe([37], [2], limits), probe([41], [3], limits)],
            limits,
        )
        .run()
        .unwrap()
    };
    let first = run();
    let repeated = run();

    assert_eq!(first.census(), repeated.census());
    for (left, right) in first.probes().iter().zip(repeated.probes()) {
        assert_eq!(left.probe_ordinal(), right.probe_ordinal());
        assert_eq!(left.modulus(), right.modulus());
        assert_eq!(left.base_parameters(), right.base_parameters());
        assert_eq!(left.chart_coordinates(), right.chart_coordinates());
        assert_eq!(left.iterations(), right.iterations());
        assert_eq!(left.outcome().kind(), right.outcome().kind());
        assert_eq!(
            left.outcome().final_requests(),
            right.outcome().final_requests()
        );
        assert_eq!(left.outcome().replayed(), right.outcome().replayed());
    }
}
