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
    CampaignError, CampaignModularProbe, OrdinarySourceIncidenceIndex,
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
    let report = tadpole_scheduler(&generator, &completed, [probe([37], [2], limits)], limits)
        .run()
        .unwrap();

    assert_eq!(report.probes().len(), 1);
    let result = &report.probes()[0];
    assert_eq!(result.probe_ordinal(), 0);
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
fn complete_empty_no_hit_census_returns_sampled_dual() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = ProbeLocalSchedulerLimits::default();
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
