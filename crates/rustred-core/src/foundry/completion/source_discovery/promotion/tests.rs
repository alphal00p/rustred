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
    ExactRuleCellGuardObstruction, ExactRuleCellPromotionDisposition, ExactRuleCellPromotionError,
    ExactRuleCellPromotionLimits, try_promote_replayed_rule_cell,
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
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
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
    let owners = ImmutableOwnerSnapshot::try_from_closed_artifact(&artifact, registry).unwrap();
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
    let ExactRuleCellPromotionDisposition::NeedsGuardedStratum {
        epoch: retained_epoch,
        circuit: retained_circuit,
        obstruction,
        ..
    } = outcome
    else {
        panic!("the boundary fixture's pivot guard must split at its interior root")
    };
    assert!(Arc::ptr_eq(&retained_epoch, &epoch));
    assert!(Arc::ptr_eq(&retained_circuit, &circuit));
    assert_eq!(
        obstruction,
        ExactRuleCellGuardObstruction::IntegerRoot {
            guard_ordinal: 0,
            position: 0,
            value: 1,
        }
    );
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
fn guard_wall_requires_semantic_routing_and_never_claims_box_ownership() {
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
        &[2],
        ExactRuleCellPromotionLimits::default(),
    )
    .unwrap();
    let ExactRuleCellPromotionDisposition::NeedsGuardedStratum {
        epoch: retained_epoch,
        circuit: retained_circuit,
        refinement,
        obstruction,
    } = outcome
    else {
        panic!("a root inside the carrier box must not produce an ordinary RuleCell owner")
    };
    assert!(Arc::ptr_eq(&retained_epoch, &epoch));
    assert!(Arc::ptr_eq(&retained_circuit, &circuit));
    assert!(!refinement.exceptional_strata().is_empty());
    assert_eq!(
        obstruction,
        ExactRuleCellGuardObstruction::IntegerRoot {
            guard_ordinal: 0,
            position: 0,
            value: 1,
        }
    );
}

#[test]
fn replay_anchor_on_guard_wall_is_retryable_and_retains_exact_authority() {
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
    let ExactRuleCellPromotionDisposition::AnchorOnGuardWall {
        epoch: retained_epoch,
        circuit: retained_circuit,
        refinement,
        guard_ordinal,
    } = outcome
    else {
        panic!("a valid exact identity must remain retryable at another anchor")
    };
    assert!(Arc::ptr_eq(&retained_epoch, &epoch));
    assert!(Arc::ptr_eq(&retained_circuit, &circuit));
    assert_eq!(refinement.parent_stratum_id(), circuit.stratum_id());
    assert_eq!(guard_ordinal, 0);
}
