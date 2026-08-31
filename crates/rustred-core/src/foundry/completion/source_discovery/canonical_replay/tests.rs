use std::cmp::Ordering;
use std::sync::Arc;

use crate::foundry::artifact::{ClosedArtifact, derive_one_loop_unit_mass_tadpole};
use crate::foundry::completion::frame::compare_exact_circuit_content;
use crate::foundry::completion::frame::exact::{
    ExactCircuitLift, ExactCircuitLimits, ExactTargetCircuit, try_lift_exact_circuit,
};
use crate::foundry::completion::frame::modular::ModularTargetQuery;
use crate::foundry::completion::source_discovery::scheduler::{
    ProbeLocalObstructionScheduler, ProbeLocalOutcome, ProbeLocalOutcomeKind,
    ProbeLocalSchedulerLimits, ProbeLocalSchedulerReport,
};
use crate::foundry::completion::stratum::{
    DecoratedStratum, ImmutableOwnerSnapshot, MaximalStratumAnchor, StratumRegistryLimits,
};
use crate::identity::{
    CompletedIbpSourceRows, IntegralShift, ParametricIbpGenerator, TranslatedSourceRequest,
};
use crate::sector::{Mask, OrderingPolicy, SectorMonotoneDomain};

use super::super::{
    AccumulatedSourceRequests, CampaignLimits, CampaignModularProbe, CampaignRequestMerge,
    ExactRuleCellPromotionDisposition, ExactRuleCellPromotionLimits, FreshTaskEpoch,
    GrowingTaskEpochState, OrdinarySourceIncidenceIndex, SourceDiscoveryLimits,
    try_promote_replayed_rule_cell,
};
use super::{
    CanonicalRebaseAttemptOutcome, CanonicalReplayDisposition, CanonicalReplayError,
    CanonicalReplayLimits, try_canonicalize_replayed_probes,
};

const PRIME: u64 = 1_000_000_007;

fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
}

fn probe(chart_coordinate: u64, limits: ProbeLocalSchedulerLimits) -> CampaignModularProbe {
    CampaignModularProbe::try_new(PRIME, [37], [chart_coordinate], limits.campaign).unwrap()
}

fn request(offset: i64) -> TranslatedSourceRequest {
    TranslatedSourceRequest::new(0, IntegralShift::try_new([offset]).unwrap())
}

fn lift_replayed_epoch(
    epoch: FreshTaskEpoch,
    generator: &ParametricIbpGenerator<'_>,
    probe: &CampaignModularProbe,
    campaign_limits: CampaignLimits,
) -> (FreshTaskEpoch, ExactTargetCircuit) {
    let circuit = {
        let query = epoch
            .try_query(generator.context(), probe, campaign_limits)
            .unwrap();
        let ModularTargetQuery::Hit(hit) = query.query() else {
            panic!("the selected tadpole frame must contain an exact modular hit")
        };
        let lift = try_lift_exact_circuit(
            generator.context(),
            hit,
            query.partition(),
            ExactCircuitLimits::default(),
        )
        .unwrap();
        let ExactCircuitLift::Replayed(circuit) = lift else {
            panic!("the selected tadpole support must replay over the exact domain")
        };
        circuit
    };
    (epoch, circuit)
}

fn tadpole_inputs(
    artifact: &ClosedArtifact,
) -> (IntegralShift, MaximalStratumAnchor, ImmutableOwnerSnapshot) {
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
    (
        target,
        MaximalStratumAnchor::try_new(stratum, registry).unwrap(),
        owners,
    )
}

#[test]
fn independently_scheduled_replays_rebase_to_one_fresh_common_epoch() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let scheduler_limits = ProbeLocalSchedulerLimits::default();
    let first_probe = probe(2, scheduler_limits);
    let second_probe = probe(3, scheduler_limits);
    let (target, anchor, owners) = tadpole_inputs(&artifact);
    let report = ProbeLocalObstructionScheduler::try_new(
        &generator,
        &completed,
        target.clone(),
        anchor.clone(),
        owners.clone(),
        OrderingPolicy::default(),
        [first_probe.clone(), second_probe.clone()],
        scheduler_limits,
    )
    .unwrap()
    .run()
    .unwrap();

    assert_eq!(report.probes().len(), 2);
    assert!(
        report
            .probes()
            .iter()
            .all(|entry| entry.outcome().kind() == ProbeLocalOutcomeKind::Replayed)
    );
    let first_epoch = report.probes()[0].outcome().epoch().unwrap();
    let second_epoch = report.probes()[1].outcome().epoch().unwrap();
    let first_old_circuit = report.probes()[0].outcome().replayed().unwrap();
    let second_old_circuit = report.probes()[1].outcome().replayed().unwrap();
    assert_eq!(first_epoch.plan(), second_epoch.plan());
    assert!(first_old_circuit.is_bound_to(first_epoch.plan()));
    assert!(second_old_circuit.is_bound_to(second_epoch.plan()));
    assert!(!first_old_circuit.is_bound_to(second_epoch.plan()));
    assert!(!second_old_circuit.is_bound_to(first_epoch.plan()));

    let disposition = try_canonicalize_replayed_probes(
        &generator,
        &completed,
        target,
        anchor,
        owners,
        OrderingPolicy::default(),
        &report,
        CanonicalReplayLimits::default(),
    )
    .unwrap();
    let CanonicalReplayDisposition::Rebased(batch) = disposition else {
        panic!("two exact tadpole nominations must rebase on their common plan")
    };

    assert_eq!(batch.epoch().plan(), first_epoch.plan());
    assert_eq!(batch.epoch().plan(), second_epoch.plan());
    assert!(!first_old_circuit.is_bound_to(batch.epoch().plan()));
    assert!(!second_old_circuit.is_bound_to(batch.epoch().plan()));
    assert_eq!(batch.attempts().len(), 2);
    assert!(
        batch
            .attempts()
            .iter()
            .all(|attempt| attempt.outcome() == &CanonicalRebaseAttemptOutcome::Replayed)
    );
    assert_eq!(batch.candidates().len(), 1);
    let candidate = &batch.candidates()[0];
    assert!(candidate.circuit().is_bound_to(batch.epoch().plan()));
    assert!(!candidate.circuit().is_bound_to(first_epoch.plan()));
    assert!(!candidate.circuit().is_bound_to(second_epoch.plan()));
    assert_eq!(candidate.anchor(), &[3]);
    assert_eq!(candidate.primary_probe(), &first_probe);
    assert_eq!(
        candidate.supporting_probes(),
        &[first_probe.clone(), second_probe.clone()]
    );
    let telemetry = batch.telemetry();
    assert_eq!(telemetry.replayed_nominations(), 2);
    assert_eq!(telemetry.rebase_attempts(), 2);
    assert_eq!(telemetry.successful_exact_lifts(), 2);
    assert_eq!(telemetry.unique_candidates(), 1);
    assert_eq!(telemetry.duplicate_exact_lifts(), 1);
}

#[test]
fn distinct_request_replays_build_the_exact_union_as_epoch_one() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let replay_limits = CanonicalReplayLimits::default();
    let scheduler_limits = ProbeLocalSchedulerLimits::default();
    let discovery_limits = SourceDiscoveryLimits::default();
    let (target, anchor, owners) = tadpole_inputs(&artifact);

    let zero_sources = generator
        .translate_completed_source_rows(
            &completed,
            [IntegralShift::try_new([0]).unwrap()],
            discovery_limits.translation,
        )
        .unwrap();
    let incidence = OrdinarySourceIncidenceIndex::try_new(&zero_sources, discovery_limits).unwrap();
    let bootstrap = incidence
        .try_nominate_target_unit(&target, discovery_limits)
        .unwrap();
    let bootstrap_requests = AccumulatedSourceRequests::try_new(
        1,
        bootstrap.requests().iter().cloned(),
        replay_limits.campaign,
    )
    .unwrap();
    assert_eq!(bootstrap_requests.requests(), &[request(0), request(1)]);
    let CampaignRequestMerge::Augmented {
        requests: union_requests,
        ..
    } = bootstrap_requests
        .try_merge_candidates([request(2)], replay_limits.campaign)
        .unwrap()
    else {
        panic!("the third tadpole translation must be a genuinely new request")
    };

    let mut bootstrap_path = GrowingTaskEpochState::new(
        target.clone(),
        anchor.clone(),
        owners.clone(),
        OrderingPolicy::default(),
    );
    let bootstrap_epoch = bootstrap_path
        .try_next(
            &generator,
            &completed,
            bootstrap_requests.clone(),
            replay_limits.campaign,
        )
        .unwrap();
    let first_probe = probe(2, scheduler_limits);
    let first_replay = lift_replayed_epoch(
        bootstrap_epoch,
        &generator,
        &first_probe,
        replay_limits.campaign,
    );

    let mut augmented_path = GrowingTaskEpochState::new(
        target.clone(),
        anchor.clone(),
        owners.clone(),
        OrderingPolicy::default(),
    );
    let superseded_bootstrap = augmented_path
        .try_next(
            &generator,
            &completed,
            bootstrap_requests.clone(),
            replay_limits.campaign,
        )
        .unwrap();
    drop(superseded_bootstrap);
    let augmented_epoch = augmented_path
        .try_next(
            &generator,
            &completed,
            union_requests.clone(),
            replay_limits.campaign,
        )
        .unwrap();
    let second_probe = probe(3, scheduler_limits);
    let second_replay = lift_replayed_epoch(
        augmented_epoch,
        &generator,
        &second_probe,
        replay_limits.campaign,
    );
    let report = ProbeLocalSchedulerReport::from_replayed_for_test([
        (first_probe, first_replay.0, first_replay.1),
        (second_probe, second_replay.0, second_replay.1),
    ]);

    let first_old_epoch = report.probes()[0].outcome().epoch().unwrap();
    let second_old_epoch = report.probes()[1].outcome().epoch().unwrap();
    let first_old_circuit = report.probes()[0].outcome().replayed().unwrap();
    let second_old_circuit = report.probes()[1].outcome().replayed().unwrap();
    assert_eq!(first_old_epoch.requests(), &bootstrap_requests);
    assert_eq!(second_old_epoch.requests(), &union_requests);
    assert_ne!(first_old_epoch.requests(), second_old_epoch.requests());
    assert_eq!(first_old_epoch.telemetry().epoch_ordinal(), 0);
    assert_eq!(second_old_epoch.telemetry().epoch_ordinal(), 1);
    assert_eq!(first_old_epoch.fixed_stratum(), anchor.initial());
    assert_ne!(second_old_epoch.fixed_stratum(), anchor.initial());
    assert!(
        second_old_epoch
            .fixed_stratum()
            .domain()
            .bounds()
            .iter()
            .zip(anchor.initial().domain().bounds())
            .all(|(inner, outer)| {
                outer.lower() <= inner.lower() && inner.upper() <= outer.upper()
            })
    );

    let disposition = try_canonicalize_replayed_probes(
        &generator,
        &completed,
        target,
        anchor,
        owners,
        OrderingPolicy::default(),
        &report,
        replay_limits,
    )
    .unwrap();
    let CanonicalReplayDisposition::Rebased(batch) = disposition else {
        panic!("the exact request union must retain at least its augmented replay")
    };

    assert_eq!(batch.epoch().telemetry().epoch_ordinal(), 1);
    assert_eq!(batch.telemetry().common_epoch_ordinal(), 1);
    assert_eq!(batch.epoch().requests(), &union_requests);
    assert_eq!(batch.telemetry().union_requests(), union_requests.len());
    assert_eq!(
        batch.epoch().fixed_stratum(),
        second_old_epoch.fixed_stratum()
    );
    assert!(
        batch
            .epoch()
            .fixed_stratum()
            .domain()
            .bounds()
            .iter()
            .zip(first_old_epoch.fixed_stratum().domain().bounds())
            .all(|(inner, outer)| {
                outer.lower() <= inner.lower() && inner.upper() <= outer.upper()
            })
    );
    assert!(!first_old_circuit.is_bound_to(batch.epoch().plan()));
    assert!(!second_old_circuit.is_bound_to(batch.epoch().plan()));
    for candidate in batch.candidates() {
        assert!(candidate.circuit().is_bound_to(batch.epoch().plan()));
        assert!(!candidate.circuit().is_bound_to(first_old_epoch.plan()));
        assert!(!candidate.circuit().is_bound_to(second_old_epoch.plan()));
    }
}

#[test]
fn canonical_candidate_promotes_to_a_sealed_rule_cell() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let scheduler_limits = ProbeLocalSchedulerLimits::default();
    let declared_probe = probe(2, scheduler_limits);
    let (target, anchor, owners) = tadpole_inputs(&artifact);
    let report = ProbeLocalObstructionScheduler::try_new(
        &generator,
        &completed,
        target.clone(),
        anchor.clone(),
        owners.clone(),
        OrderingPolicy::default(),
        [declared_probe],
        scheduler_limits,
    )
    .unwrap()
    .run()
    .unwrap();
    let CanonicalReplayDisposition::Rebased(batch) = try_canonicalize_replayed_probes(
        &generator,
        &completed,
        target,
        anchor,
        owners,
        OrderingPolicy::default(),
        &report,
        CanonicalReplayLimits::default(),
    )
    .unwrap() else {
        panic!("the canonical tadpole replay must expose an exact candidate")
    };
    let candidate = &batch.candidates()[0];
    let retained_epoch = batch.epoch().clone();
    let retained_circuit = candidate.circuit().clone();
    let outcome = try_promote_replayed_rule_cell(
        generator.context(),
        retained_epoch.clone(),
        retained_circuit.clone(),
        candidate.anchor(),
        ExactRuleCellPromotionLimits::default(),
    )
    .unwrap();
    let ExactRuleCellPromotionDisposition::Admitted(admitted) = outcome else {
        panic!("the canonical tadpole anchor must admit a sealed rule cell")
    };

    assert!(Arc::ptr_eq(admitted.epoch(), &retained_epoch));
    assert!(Arc::ptr_eq(admitted.circuit(), &retained_circuit));
    assert_eq!(
        admitted.cell().application_domain(),
        batch.epoch().fixed_stratum().domain()
    );
    assert_eq!(
        admitted.cell().rule().pivot().values(),
        batch.epoch().target_shift().values()
    );
}

#[test]
fn canonical_replay_is_deterministic_under_reversed_scheduler_order() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let scheduler_limits = ProbeLocalSchedulerLimits::default();
    let lower_probe = probe(2, scheduler_limits);
    let upper_probe = probe(3, scheduler_limits);
    let (target, anchor, owners) = tadpole_inputs(&artifact);
    let run_scheduler = |probes| {
        ProbeLocalObstructionScheduler::try_new(
            &generator,
            &completed,
            target.clone(),
            anchor.clone(),
            owners.clone(),
            OrderingPolicy::default(),
            probes,
            scheduler_limits,
        )
        .unwrap()
        .run()
        .unwrap()
    };
    let forward_report = run_scheduler([lower_probe.clone(), upper_probe.clone()]);
    let reversed_report = run_scheduler([upper_probe, lower_probe]);
    let canonicalize = |report| {
        try_canonicalize_replayed_probes(
            &generator,
            &completed,
            target.clone(),
            anchor.clone(),
            owners.clone(),
            OrderingPolicy::default(),
            report,
            CanonicalReplayLimits::default(),
        )
        .unwrap()
    };
    let CanonicalReplayDisposition::Rebased(forward) = canonicalize(&forward_report) else {
        panic!("forward schedule must produce a canonical replay batch")
    };
    let CanonicalReplayDisposition::Rebased(reversed) = canonicalize(&reversed_report) else {
        panic!("reversed schedule must produce a canonical replay batch")
    };

    assert_eq!(forward.telemetry(), reversed.telemetry());
    assert_eq!(forward.attempts(), reversed.attempts());
    assert_eq!(forward.epoch().requests(), reversed.epoch().requests());
    assert_eq!(forward.epoch().plan(), reversed.epoch().plan());
    assert_eq!(forward.candidates().len(), reversed.candidates().len());
    for (left, right) in forward.candidates().iter().zip(reversed.candidates()) {
        assert_eq!(left.anchor(), right.anchor());
        assert_eq!(left.primary_probe(), right.primary_probe());
        assert_eq!(left.supporting_probes(), right.supporting_probes());
        assert_eq!(
            compare_exact_circuit_content(left.circuit(), right.circuit()),
            Ordering::Equal
        );
        assert!(left.circuit().is_bound_to(forward.epoch().plan()));
        assert!(right.circuit().is_bound_to(reversed.epoch().plan()));
        assert!(!left.circuit().is_bound_to(reversed.epoch().plan()));
        assert!(!right.circuit().is_bound_to(forward.epoch().plan()));
    }
}

#[test]
fn report_without_exact_replays_has_no_canonical_nominations() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let mut scheduler_limits = ProbeLocalSchedulerLimits::default();
    scheduler_limits.max_iterations_per_probe = 0;
    let declared_probe = probe(2, scheduler_limits);
    let (target, anchor, owners) = tadpole_inputs(&artifact);
    let report = ProbeLocalObstructionScheduler::try_new(
        &generator,
        &completed,
        target.clone(),
        anchor.clone(),
        owners.clone(),
        OrderingPolicy::default(),
        [declared_probe],
        scheduler_limits,
    )
    .unwrap()
    .run()
    .unwrap();
    assert!(matches!(
        report.probes()[0].outcome(),
        ProbeLocalOutcome::BudgetStop { .. }
    ));

    let disposition = try_canonicalize_replayed_probes(
        &generator,
        &completed,
        target,
        anchor,
        owners,
        OrderingPolicy::default(),
        &report,
        CanonicalReplayLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        disposition,
        CanonicalReplayDisposition::NoReplayedNominations
    ));
}

#[test]
fn replay_nomination_cap_fails_with_typed_bounded_resource_error() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let scheduler_limits = ProbeLocalSchedulerLimits::default();
    let (target, anchor, owners) = tadpole_inputs(&artifact);
    let report = ProbeLocalObstructionScheduler::try_new(
        &generator,
        &completed,
        target.clone(),
        anchor.clone(),
        owners.clone(),
        OrderingPolicy::default(),
        [probe(2, scheduler_limits), probe(3, scheduler_limits)],
        scheduler_limits,
    )
    .unwrap()
    .run()
    .unwrap();
    let mut replay_limits = CanonicalReplayLimits::default();
    replay_limits.max_replayed_nominations = 1;

    assert_eq!(
        try_canonicalize_replayed_probes(
            &generator,
            &completed,
            target,
            anchor,
            owners,
            OrderingPolicy::default(),
            &report,
            replay_limits,
        )
        .unwrap_err(),
        CanonicalReplayError::ResourceLimit {
            resource: "replayed probe nominations",
            requested: 2,
            limit: 1,
        }
    );
}

#[test]
fn supporting_probe_cap_is_enforced_during_exact_content_deduplication() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let scheduler_limits = ProbeLocalSchedulerLimits::default();
    let (target, anchor, owners) = tadpole_inputs(&artifact);
    let report = ProbeLocalObstructionScheduler::try_new(
        &generator,
        &completed,
        target.clone(),
        anchor.clone(),
        owners.clone(),
        OrderingPolicy::default(),
        [probe(2, scheduler_limits), probe(3, scheduler_limits)],
        scheduler_limits,
    )
    .unwrap()
    .run()
    .unwrap();
    let mut replay_limits = CanonicalReplayLimits::default();
    replay_limits.max_supporting_probe_references = 1;

    assert_eq!(
        try_canonicalize_replayed_probes(
            &generator,
            &completed,
            target,
            anchor,
            owners,
            OrderingPolicy::default(),
            &report,
            replay_limits,
        )
        .unwrap_err(),
        CanonicalReplayError::ResourceLimit {
            resource: "supporting probe references",
            requested: 2,
            limit: 1,
        }
    );
}
