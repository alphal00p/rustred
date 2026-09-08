use crate::family::IntegralKey;
use crate::foundry::artifact::derive_one_loop_unit_mass_tadpole;
use crate::foundry::completion::source_discovery::{
    CampaignLimits, CampaignModularProbe, ExactRuleCellPromotionDisposition,
    ExactRuleCellPromotionLimits,
};
use crate::foundry::completion::stratum::{
    DecoratedStratum, ImmutableOwnerSnapshot, StratumRegistryLimits,
};
use crate::identity::{
    CompletedIbpSourceRows, IntegralShift, ParametricIbpGenerator, TranslatedSourceRequest,
};
use crate::sector::{Mask, OrderingPolicy, SectorMonotoneDomain};

use super::super::{
    SpiredCase, SpiredCoordinateFace, SpiredExecutionCase, SpiredStreamingDiscovery,
    SpiredStreamingLimits,
};
use super::{
    SpiredCompactLift, SpiredCompactLiftLimits, try_lift_spired_compact_support,
    try_promote_spired_replayed_rule_cell,
};

const PRIME: u64 = 1_000_000_007;

fn request(offset: i64) -> TranslatedSourceRequest {
    TranslatedSourceRequest::new(0, IntegralShift::try_new([offset]).unwrap())
}

fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
}

#[test]
fn k1_streaming_support_replays_and_promotes_with_strict_descent_authority() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);

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
    let case = SpiredExecutionCase::try_new(
        SpiredCase::CoordinateFace(SpiredCoordinateFace::new(stratum)),
        target.clone(),
        OrderingPolicy::default(),
        owners,
    )
    .unwrap();
    let probe = CampaignModularProbe::try_new(PRIME, [37], [2], CampaignLimits::default()).unwrap();
    let request = request(0);
    let mut streaming = SpiredStreamingDiscovery::try_new(
        generator.context(),
        &completed,
        &case,
        &probe,
        SpiredStreamingLimits::default(),
    )
    .unwrap();
    let hit = streaming
        .try_consume_chunk(std::slice::from_ref(&request))
        .unwrap()
        .expect("the real shifted evaluator/classifier/kernel must find K=1 support");
    assert_eq!(hit.support(), std::slice::from_ref(&request));

    let lifted = try_lift_spired_compact_support(
        &case,
        &generator,
        &completed,
        &hit,
        streaming.probe(),
        SpiredCompactLiftLimits::default(),
    )
    .unwrap();
    let SpiredCompactLift::Replayed(replayed) = lifted else {
        panic!("the canonical K=1 support must exactly replay")
    };
    let epoch = replayed.epoch().clone();
    let circuit = replayed.circuit().clone();
    assert_eq!(replayed.probe(), &probe);
    assert_eq!(epoch.requests().requests(), std::slice::from_ref(&request));
    assert!(circuit.is_bound_to(epoch.plan()));
    assert_eq!(circuit.target_shift(), &target);
    assert!(!circuit.source_combination().is_empty());
    assert_eq!(
        circuit.replay().source_contributions(),
        circuit.source_combination().len()
    );

    let promoted = try_promote_spired_replayed_rule_cell(
        generator.context(),
        replayed,
        ExactRuleCellPromotionLimits::default(),
    )
    .unwrap();
    let ExactRuleCellPromotionDisposition::Admitted(candidate) = promoted else {
        panic!("the exact K=1 replay must promote to an executable RuleCell")
    };
    assert!(std::sync::Arc::ptr_eq(candidate.epoch(), &epoch));
    assert!(std::sync::Arc::ptr_eq(candidate.circuit(), &circuit));
    assert!(candidate.circuit().is_bound_to(candidate.epoch().plan()));
    assert_eq!(candidate.cell().rule().pivot().values(), target.values());
    assert_eq!(
        candidate.cell().rule().right_hand_side().len(),
        candidate.circuit().residual_terms().len()
    );
    assert!(
        candidate
            .cell()
            .terms()
            .iter()
            .all(|term| term.descent().verify())
    );
    assert_eq!(
        candidate
            .cell()
            .assignment_for_target(&IntegralKey::try_new([3]).unwrap())
            .unwrap(),
        Some(vec![2])
    );
}
