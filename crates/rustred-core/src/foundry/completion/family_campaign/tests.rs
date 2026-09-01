use crate::algebra::CoefficientContext;
use crate::family::presentation::{
    AuxiliaryDenominator, CommonMassScale, DenominatorRole, FamilyConventions, FamilyPresentation,
    MetricConvention, MomentumCombination, MomentumRouting, PhysicalPropagator,
    PropagatorConvention,
};
use crate::family::{AffineDenominator, IntegralFamily};
use crate::sector::OrderingPolicy;
use crate::sector::symmetry::permutation::compile;
use crate::sector::symmetry::{
    CanonicalizationLimits, Canonicalizer, CoefficientMatrix, Limits as SymmetryLimits,
    MomentumMap, verify,
};

use super::{CompletePhysicalContractionGoal, FamilyCoverageError, FamilyCoverageLimits};

fn presentation(name: &str, physical_slots: &[usize]) -> FamilyPresentation {
    let context = CoefficientContext::try_new(["d"]).unwrap();
    let zero = context.zero();
    let one = context.one();
    let minus_one = context.integer(-1);
    // Scalar coordinates are (k1^2, k1.k2, k2^2). The auxiliary row is
    // deliberately fixed by k1 <-> k2, while the two physical rows swap.
    let family = IntegralFamily::new(
        name,
        vec!["k1".to_owned(), "k2".to_owned()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(
                minus_one.clone(),
                vec![one.clone(), zero.clone(), zero.clone()],
            ),
            AffineDenominator::new(minus_one, vec![zero.clone(), zero.clone(), one.clone()]),
            AffineDenominator::new(zero.clone(), vec![zero.clone(), one.clone(), zero.clone()]),
        ],
        Vec::new(),
        vec![zero.clone(), zero.clone(), zero],
    )
    .unwrap();
    let roles = (0..3)
        .map(|slot| match (slot, physical_slots.contains(&slot)) {
            (0, true) => DenominatorRole::Physical(PhysicalPropagator::new(
                "D1".to_owned(),
                MomentumCombination::new(vec![context.one(), context.zero()], Vec::new()),
                context.one(),
            )),
            (1, true) => DenominatorRole::Physical(PhysicalPropagator::new(
                "D2".to_owned(),
                MomentumCombination::new(vec![context.zero(), context.one()], Vec::new()),
                context.one(),
            )),
            _ => DenominatorRole::Auxiliary(AuxiliaryDenominator::new(format!("A{}", slot + 1))),
        })
        .collect();
    let routing = MomentumRouting::new(
        vec!["source-k1".to_owned(), "source-k2".to_owned()],
        Vec::new(),
        vec![
            vec![context.one(), context.zero()],
            vec![context.zero(), context.one()],
        ],
        vec![Vec::new(), Vec::new()],
        Vec::new(),
    );
    FamilyPresentation::try_new(
        family,
        roles,
        routing,
        FamilyConventions::new(
            MetricConvention::Euclidean,
            PropagatorConvention::MOMENTUM_SQUARED_MINUS_MASS_SQUARED,
        ),
        (!physical_slots.is_empty()).then(|| CommonMassScale::new(context.one())),
    )
    .unwrap()
}

fn physical_swap_canonicalizer(presentation: &FamilyPresentation) -> Canonicalizer {
    let family = presentation.family();
    let context = family.coefficient_context();
    let momentum = MomentumMap::new(
        CoefficientMatrix::try_new(
            2,
            2,
            [0, 1, 1, 0].into_iter().map(|entry| context.integer(entry)),
        )
        .unwrap(),
        CoefficientMatrix::try_new(2, 0, []).unwrap(),
        CoefficientMatrix::try_new(0, 0, []).unwrap(),
    );
    let verified = verify(family, family, momentum, SymmetryLimits::default()).unwrap();
    let permutation = compile(family, verified).unwrap();
    Canonicalizer::try_new(
        OrderingPolicy::default(),
        [permutation],
        CanonicalizationLimits::default(),
    )
    .unwrap()
}

#[test]
fn role_preserving_symmetry_keeps_auxiliary_slots_inactive_and_counts_exact_orbits() {
    let presentation = presentation("role-preserving-auxiliary", &[0, 1]);
    let canonicalizer = physical_swap_canonicalizer(&presentation);
    let goal = CompletePhysicalContractionGoal::try_new(&presentation).unwrap();
    let first = goal
        .try_plan(&canonicalizer, FamilyCoverageLimits::default())
        .unwrap();
    let repeated = goal
        .try_plan(&canonicalizer, FamilyCoverageLimits::default())
        .unwrap();

    assert_eq!(first, repeated);
    assert_eq!(goal.physical_slot_count(), 2);
    assert_eq!(goal.maximal_sector().active_bits(), &[true, true, false]);
    assert_eq!(
        first.raw_sector_count(),
        1_usize << goal.physical_slot_count()
    );
    assert_eq!(
        first
            .required_orbits()
            .iter()
            .map(|orbit| orbit.raw_sector_count())
            .sum::<usize>(),
        4
    );
    assert_eq!(
        first
            .required_orbits()
            .iter()
            .map(|orbit| (orbit.corner().powers().to_vec(), orbit.raw_sector_count()))
            .collect::<Vec<_>>(),
        [(vec![0, 0, 0], 1), (vec![0, 1, 0], 2), (vec![1, 1, 0], 1),]
    );
    assert!(
        first
            .required_orbits()
            .iter()
            .all(|orbit| { !orbit.sector().active_bits()[2] && orbit.corner().powers()[2] == 0 })
    );
}

#[test]
fn complete_downset_resource_limits_fail_at_exact_one_below_boundaries() {
    let presentation = presentation("coverage-limits", &[0, 1]);
    let canonicalizer = physical_swap_canonicalizer(&presentation);
    let goal = CompletePhysicalContractionGoal::try_new(&presentation).unwrap();

    assert_eq!(
        goal.try_plan(
            &canonicalizer,
            FamilyCoverageLimits {
                max_physical_contractions: 3,
                max_sector_orbits: 3,
            },
        ),
        Err(FamilyCoverageError::ResourceLimit {
            resource: "physical contraction masks",
            requested: 4,
            limit: 3,
        })
    );
    assert_eq!(
        goal.try_plan(
            &canonicalizer,
            FamilyCoverageLimits {
                max_physical_contractions: 4,
                max_sector_orbits: 2,
            },
        ),
        Err(FamilyCoverageError::ResourceLimit {
            resource: "canonical sector orbits",
            requested: 3,
            limit: 2,
        })
    );
}

#[test]
fn goal_and_canonicalizer_authorities_reject_missing_physical_rows_and_wrong_family() {
    let no_physical = presentation("no-physical-rows", &[]);
    assert_eq!(
        CompletePhysicalContractionGoal::try_new(&no_physical),
        Err(FamilyCoverageError::NoPhysicalPropagators)
    );

    let canonicalizer_presentation = presentation("canonicalizer-family", &[0, 1]);
    let canonicalizer = physical_swap_canonicalizer(&canonicalizer_presentation);
    let foreign_presentation = presentation("foreign-family", &[0, 1]);
    let foreign_goal = CompletePhysicalContractionGoal::try_new(&foreign_presentation).unwrap();
    assert_eq!(
        foreign_goal.try_plan(&canonicalizer, FamilyCoverageLimits::default()),
        Err(FamilyCoverageError::WrongCanonicalizerFamily)
    );
}

#[test]
fn physical_downset_size_overflow_is_typed_without_allocating() {
    let physical_slot_count = usize::BITS as usize;
    assert_eq!(
        super::plan::checked_downset_size(physical_slot_count),
        Err(FamilyCoverageError::PhysicalContractionCountOverflow {
            physical_slot_count,
        })
    );
}
