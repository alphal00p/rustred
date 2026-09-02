use std::sync::Arc;

use crate::algebra::IndexedCoefficientContext;
use crate::family::IntegralKey;
use crate::foundry::artifact::{ClosedArtifact, derive_one_loop_unit_mass_tadpole};
use crate::foundry::completion::source_discovery::scheduler::{
    ProbeLocalObstructionScheduler, ProbeLocalOutcome, ProbeLocalSchedulerLimits,
};
use crate::foundry::completion::stratum::{
    DecoratedStratum, ImmutableOwnerSnapshot, MaximalStratumAnchor, StratumRegistryLimits,
};
use crate::identity::{CompletedIbpSourceRows, IntegralShift, ParametricIbpGenerator};
use crate::sector::{Mask, OrderingPolicy, SectorMonotoneDomain};

use super::{
    ExactRuleCellPromotionDisposition, ExactRuleCellPromotionError, ExactRuleCellPromotionLimits,
    try_promote_replayed_rule_cell,
};

const PRIME: u64 = 1_000_000_007;

fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
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

fn replayed_tadpole() -> (
    IndexedCoefficientContext,
    Arc<crate::foundry::completion::source_discovery::FreshTaskEpoch>,
    Arc<crate::foundry::completion::frame::exact::ExactTargetCircuit>,
) {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let context = generator.context().clone();
    let completed = complete_ordinary(&generator);
    let limits = ProbeLocalSchedulerLimits::default();
    let (target, stratum, owners) = tadpole_inputs(&artifact);
    let probe = crate::foundry::completion::source_discovery::CampaignModularProbe::try_new(
        PRIME,
        [37],
        [2],
        limits.campaign,
    )
    .unwrap();
    let report = ProbeLocalObstructionScheduler::try_new(
        &generator,
        &completed,
        target,
        stratum,
        owners,
        OrderingPolicy::default(),
        [probe],
        limits,
    )
    .unwrap()
    .run()
    .unwrap();
    let probe = Vec::from(report.into_probes()).pop().unwrap();
    let ProbeLocalOutcome::Replayed { epoch, circuit } = probe.into_outcome() else {
        panic!("the massive tadpole bootstrap must replay exactly")
    };
    (context, Arc::new(epoch), Arc::new(circuit))
}

fn replayed_owned_boundary_tadpole() -> (
    IndexedCoefficientContext,
    Arc<crate::foundry::completion::source_discovery::FreshTaskEpoch>,
    Arc<crate::foundry::completion::frame::exact::ExactTargetCircuit>,
) {
    let artifact = Arc::new(derive_one_loop_unit_mass_tadpole().unwrap());
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let context = generator.context().clone();
    let completed = complete_ordinary(&generator);
    let limits = ProbeLocalSchedulerLimits::default();
    let registry = StratumRegistryLimits::default();
    let target = IntegralShift::try_new([0]).unwrap();
    let domain = SectorMonotoneDomain::try_maximal_for_rule(
        Mask::try_new([true]).unwrap(),
        target.values(),
        &[vec![-1], vec![0], vec![1]],
    )
    .unwrap();
    let stratum = DecoratedStratum::try_guard_blind(
        artifact.family_fingerprint(),
        artifact.context_fingerprint(),
        domain,
        registry,
    )
    .unwrap();
    let owners =
        ImmutableOwnerSnapshot::try_from_closed_artifact(Arc::clone(&artifact), registry).unwrap();
    assert!(owners.owner_count() > 0);
    let probe = crate::foundry::completion::source_discovery::CampaignModularProbe::try_new(
        PRIME,
        [37],
        [2],
        limits.campaign,
    )
    .unwrap();
    let report = ProbeLocalObstructionScheduler::try_new(
        &generator,
        &completed,
        target,
        MaximalStratumAnchor::try_new(stratum, registry).unwrap(),
        owners,
        OrderingPolicy::default(),
        [probe],
        limits,
    )
    .unwrap()
    .run()
    .unwrap();
    let probe = Vec::from(report.into_probes()).pop().unwrap();
    let ProbeLocalOutcome::Replayed { epoch, circuit } = probe.into_outcome() else {
        panic!("the translated tadpole boundary target must replay exactly")
    };
    (context, Arc::new(epoch), Arc::new(circuit))
}

#[test]
fn scheduler_owned_replay_promotes_to_one_sealed_executable_candidate() {
    let (context, epoch, circuit) = replayed_tadpole();
    let outcome = try_promote_replayed_rule_cell(
        &context,
        epoch.clone(),
        circuit.clone(),
        &[2],
        ExactRuleCellPromotionLimits::default(),
    )
    .unwrap();
    let ExactRuleCellPromotionDisposition::Admitted(candidate) = outcome else {
        panic!("the tadpole guard must be total on its complete application box")
    };

    assert!(Arc::ptr_eq(candidate.epoch(), &epoch));
    assert!(Arc::ptr_eq(candidate.circuit(), &circuit));
    assert_eq!(
        Arc::as_ptr(candidate.cell_owner()),
        candidate.cell() as *const _
    );
    assert_eq!(
        candidate.guard_refinement().parent_stratum_id(),
        circuit.stratum_id()
    );
    let cell = candidate.cell();
    assert_eq!(cell.application_domain(), epoch.fixed_stratum().domain());
    assert_eq!(
        cell.rule().pivot().values(),
        circuit.target_shift().values()
    );
    assert_eq!(
        cell.rule().right_hand_side().len(),
        circuit.residual_terms().len()
    );
    assert_eq!(cell.sources().len(), cell.sources().provenance().len());
    assert!(!cell.sources().is_empty());
    assert!(cell.terms().iter().all(|term| term.descent().verify()));
    assert_eq!(
        cell.assignment_for_target(&IntegralKey::try_new([3]).unwrap())
            .unwrap(),
        Some(vec![2])
    );
}

#[test]
fn nonempty_owner_snapshot_rejoins_a_retained_proper_subsector_witness() {
    let (context, epoch, circuit) = replayed_owned_boundary_tadpole();
    assert!(
        circuit
            .residual_terms()
            .iter()
            .any(|term| !term.proper_subsector_owners().is_empty()),
        "the boundary fixture must retain a lower-sector owner witness"
    );

    let outcome = try_promote_replayed_rule_cell(
        &context,
        epoch.clone(),
        circuit.clone(),
        &[2],
        ExactRuleCellPromotionLimits::default(),
    )
    .unwrap();
    let ExactRuleCellPromotionDisposition::Admitted(candidate) = outcome else {
        panic!("the endpoint guard root must publish its one rectangular complement")
    };
    assert!(Arc::ptr_eq(candidate.epoch(), &epoch));
    assert!(Arc::ptr_eq(candidate.circuit(), &circuit));
    let split = candidate
        .guard_domain_split()
        .expect("the genuine final-target guard must retain split evidence");
    assert_eq!(
        (split.guard_ordinal(), split.position(), split.value()),
        (0, 0, 1)
    );
    assert_eq!(
        split.admitted_domain(),
        candidate.cell().application_domain()
    );
    assert_eq!(split.admitted_domain().bounds()[0].lower(), 2);
    assert_eq!(
        split.admitted_domain().bounds()[0].upper(),
        epoch.fixed_stratum().domain().bounds()[0].upper()
    );
    assert_eq!(split.exceptional_domain().bounds()[0].lower(), 1);
    assert_eq!(split.exceptional_domain().bounds()[0].upper(), 1);
    assert!(split.deferred_guard_free_domain().is_none());
}

#[test]
fn structurally_equal_fresh_epoch_cannot_reinterpret_old_physical_ordinals() {
    let (context, _first_epoch, first_circuit) = replayed_tadpole();
    let (_, second_epoch, _) = replayed_tadpole();
    assert_eq!(first_circuit.target_shift(), second_epoch.target_shift());
    assert!(!first_circuit.is_bound_to(second_epoch.plan()));

    assert!(matches!(
        try_promote_replayed_rule_cell(
            &context,
            second_epoch,
            first_circuit,
            &[2],
            ExactRuleCellPromotionLimits::default(),
        ),
        Err(ExactRuleCellPromotionError::WrongPhysicalPlan)
    ));
}

#[test]
fn elimination_path_guard_is_pruned_before_rule_cell_ownership() {
    let (context, epoch, circuit) = replayed_tadpole();
    let mut circuit = Arc::try_unwrap(circuit).unwrap();
    let guard = context
        .sub(&context.index(0).unwrap(), &context.one())
        .unwrap();
    let injected_guard = context
        .numerator_condition_with_limits(&guard, Default::default())
        .unwrap();
    circuit.replace_first_guard_polynomial_for_test(injected_guard.clone());
    let circuit = Arc::new(circuit);

    let outcome = try_promote_replayed_rule_cell(
        &context,
        epoch.clone(),
        circuit.clone(),
        &[2],
        ExactRuleCellPromotionLimits::default(),
    )
    .unwrap();
    let ExactRuleCellPromotionDisposition::Admitted(candidate) = outcome else {
        panic!("an elimination-path guard absent from the cleared consequence must be pruned")
    };
    assert!(Arc::ptr_eq(candidate.epoch(), &epoch));
    assert!(Arc::ptr_eq(candidate.circuit(), &circuit));
    assert!(candidate.cleared().is_bound_to(&circuit));
    let telemetry = candidate.cleared().guard_telemetry();
    assert_eq!(telemetry.before_unique(), circuit.nonzero_guards().len());
    assert!(telemetry.after_unique() < telemetry.before_unique());
    assert!(
        candidate
            .cleared()
            .semantic_guards()
            .iter()
            .all(|guard| guard.polynomial() != &injected_guard),
        "the synthetic reducer-pivot guard must not survive semantic minimization"
    );
}

#[test]
fn replay_anchor_on_pruned_elimination_guard_is_directly_admitted() {
    let (context, epoch, circuit) = replayed_tadpole();
    let mut circuit = Arc::try_unwrap(circuit).unwrap();
    let guard = context
        .sub(&context.index(0).unwrap(), &context.one())
        .unwrap();
    circuit.replace_first_guard_polynomial_for_test(
        context
            .numerator_condition_with_limits(&guard, Default::default())
            .unwrap(),
    );
    let circuit = Arc::new(circuit);

    let outcome = try_promote_replayed_rule_cell(
        &context,
        epoch.clone(),
        circuit.clone(),
        &[1],
        ExactRuleCellPromotionLimits::default(),
    )
    .unwrap();
    let ExactRuleCellPromotionDisposition::Admitted(candidate) = outcome else {
        panic!("a pruned elimination-path wall must not reject the replay anchor")
    };
    assert!(Arc::ptr_eq(candidate.epoch(), &epoch));
    assert!(Arc::ptr_eq(candidate.circuit(), &circuit));
    assert!(candidate.cleared().is_bound_to(&circuit));
}
