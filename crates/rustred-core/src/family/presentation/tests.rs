use crate::algebra::{Coefficient, CoefficientContext, ExactAlgebraLimits};
use crate::family::{AffineDenominator, IntegralFamily};

use super::*;

mod limits;

fn conventions() -> FamilyConventions {
    FamilyConventions::new(
        MetricConvention::Euclidean,
        PropagatorConvention::MOMENTUM_SQUARED_PLUS_MASS_SQUARED,
    )
}

fn identity_routing(
    context: &CoefficientContext,
    loops: usize,
    externals: usize,
) -> MomentumRouting {
    let identity = |size: usize| {
        (0..size)
            .map(|row| {
                (0..size)
                    .map(|column| {
                        if row == column {
                            context.one()
                        } else {
                            context.zero()
                        }
                    })
                    .collect()
            })
            .collect()
    };
    MomentumRouting::new(
        (0..loops).map(|index| format!("source-k{index}")).collect(),
        (0..externals)
            .map(|index| format!("source-p{index}"))
            .collect(),
        identity(loops),
        (0..loops)
            .map(|_| vec![context.zero(); externals])
            .collect(),
        identity(externals),
    )
}

fn vacuum_fixture(loops: usize) -> (IntegralFamily, CoefficientContext, Vec<DenominatorRole>) {
    let context = CoefficientContext::new(["d", "m2", "other"]);
    let mass = context.parameter("m2").unwrap();
    let coordinates = loops * (loops + 1) / 2;
    // Compute coordinate positions directly from the declared upper-triangle
    // order so the fixture remains independent of IntegralFamily internals.
    let mut diagonal_coordinates = Vec::new();
    let mut position = 0usize;
    for left in 0..loops {
        for right in left..loops {
            if left == right {
                diagonal_coordinates.push(position);
            }
            position += 1;
        }
    }

    let row = |coordinate: usize, constant: Coefficient| {
        AffineDenominator::new(
            constant,
            (0..coordinates)
                .map(|candidate| {
                    if candidate == coordinate {
                        context.one()
                    } else {
                        context.zero()
                    }
                })
                .collect(),
        )
    };
    let mut denominators = Vec::new();
    let mut roles = Vec::new();
    for (loop_index, &coordinate) in diagonal_coordinates.iter().enumerate() {
        denominators.push(row(coordinate, mass.clone()));
        roles.push(DenominatorRole::Physical(PhysicalPropagator::new(
            format!("D{loop_index}"),
            MomentumCombination::new(
                (0..loops)
                    .map(|candidate| {
                        if candidate == loop_index {
                            context.one()
                        } else {
                            context.zero()
                        }
                    })
                    .collect(),
                Vec::new(),
            ),
            mass.clone(),
        )));
    }
    for coordinate in 0..coordinates {
        if diagonal_coordinates.contains(&coordinate) {
            continue;
        }
        denominators.push(row(coordinate, context.zero()));
        roles.push(DenominatorRole::Auxiliary(AuxiliaryDenominator::new(
            format!("ISP{coordinate}"),
        )));
    }
    let family = IntegralFamily::new(
        format!("generic-{loops}-loop-vacuum"),
        (0..loops).map(|index| format!("k{index}")).collect(),
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        denominators,
        Vec::new(),
        vec![context.zero(); coordinates],
    )
    .unwrap();
    (family, context, roles)
}

fn external_fixture(shifted_physical: bool) -> (IntegralFamily, CoefficientContext) {
    let context = CoefficientContext::new(["d", "m2", "s"]);
    let mass = context.parameter("m2").unwrap();
    let invariant = context.parameter("s").unwrap();
    let constant = if shifted_physical {
        context
            .try_add(&mass, &invariant, ExactAlgebraLimits::default())
            .unwrap()
    } else {
        mass
    };
    let two = context.integer(2);
    let family = IntegralFamily::new(
        if shifted_physical {
            "shifted-propagator"
        } else {
            "spectator-external"
        },
        vec!["k".into()],
        vec!["p".into()],
        context.clone(),
        context.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(
                constant,
                vec![
                    context.one(),
                    if shifted_physical {
                        two
                    } else {
                        context.zero()
                    },
                ],
            ),
            AffineDenominator::new(context.zero(), vec![context.zero(), context.one()]),
        ],
        vec![vec![invariant]],
        vec![context.zero(), context.zero()],
    )
    .unwrap();
    (family, context)
}

fn external_roles(context: &CoefficientContext, shifted_physical: bool) -> Vec<DenominatorRole> {
    vec![
        DenominatorRole::Physical(PhysicalPropagator::new(
            "D".to_owned(),
            MomentumCombination::new(
                vec![context.one()],
                vec![if shifted_physical {
                    context.one()
                } else {
                    context.zero()
                }],
            ),
            context.parameter("m2").unwrap(),
        )),
        DenominatorRole::Auxiliary(AuxiliaryDenominator::new("ISP-kp".to_owned())),
    ]
}

fn one_loop_presentation_with_limits(
    limits: FamilyPresentationLimits,
) -> Result<FamilyPresentation, FamilyPresentationError> {
    let (family, context, roles) = vacuum_fixture(1);
    FamilyPresentation::try_new_with_limits(
        family,
        roles,
        identity_routing(&context, 1, 0),
        conventions(),
        Some(CommonMassScale::new(context.parameter("m2").unwrap())),
        limits,
    )
}

#[test]
fn proof_is_topology_neutral_across_loop_counts_and_auxiliary_isps() {
    for loops in [1, 2, 4] {
        let (family, context, roles) = vacuum_fixture(loops);
        let auxiliary_count = roles
            .iter()
            .filter(|role| matches!(role, DenominatorRole::Auxiliary(_)))
            .count();
        let presentation = FamilyPresentation::try_new(
            family,
            roles,
            identity_routing(&context, loops, 0),
            conventions(),
            Some(CommonMassScale::new(context.parameter("m2").unwrap())),
        )
        .unwrap();
        let evidence = presentation.single_scale_vacuum_evidence().unwrap();
        assert_eq!(evidence.physical_denominators().count(), loops);
        assert_eq!(
            presentation
                .denominator_roles()
                .iter()
                .filter(|role| matches!(role, DenominatorRole::Auxiliary(_)))
                .count(),
            auxiliary_count
        );
        assert_eq!(
            evidence.common_mass_scale().scale_squared(),
            &context.parameter("m2").unwrap()
        );
    }
}

#[test]
fn spectator_external_coordinates_do_not_invalidate_vacuum_evidence() {
    let (family, context) = external_fixture(false);
    let presentation = FamilyPresentation::try_new(
        family,
        external_roles(&context, false),
        identity_routing(&context, 1, 1),
        conventions(),
        Some(CommonMassScale::new(context.parameter("m2").unwrap())),
    )
    .unwrap();
    assert_eq!(presentation.family().external_count(), 1);
    assert!(presentation.single_scale_vacuum_evidence().is_ok());
}

#[test]
fn physical_external_shift_is_typed_ineligibility_not_presentation_rejection() {
    let (family, context) = external_fixture(true);
    let presentation = FamilyPresentation::try_new(
        family,
        external_roles(&context, true),
        identity_routing(&context, 1, 1),
        conventions(),
        Some(CommonMassScale::new(context.parameter("m2").unwrap())),
    )
    .unwrap();
    assert!(matches!(
        presentation.single_scale_vacuum_evidence(),
        Err(SingleScaleVacuumIneligibility::PhysicalExternalShift {
            denominator: 0,
            external: 0,
        })
    ));
}

#[test]
fn common_scale_and_physical_replay_are_exact() {
    let (family, context, roles) = vacuum_fixture(1);
    let result = FamilyPresentation::try_new(
        family,
        roles,
        identity_routing(&context, 1, 0),
        conventions(),
        Some(CommonMassScale::new(context.parameter("other").unwrap())),
    );
    assert!(matches!(
        result,
        Err(FamilyPresentationError::PhysicalMassOutsideCommonScale { denominator: 0 })
    ));

    let (family, context, mut roles) = vacuum_fixture(1);
    roles[0] = DenominatorRole::Physical(PhysicalPropagator::new(
        "D0".to_owned(),
        MomentumCombination::new(vec![context.one()], Vec::new()),
        context.zero(),
    ));
    let result = FamilyPresentation::try_new(
        family,
        roles,
        identity_routing(&context, 1, 0),
        conventions(),
        None,
    );
    assert!(matches!(
        result,
        Err(FamilyPresentationError::PhysicalDenominatorMismatch {
            denominator: 0,
            component: PresentationDenominatorComponent::Constant,
        })
    ));
}

#[test]
fn presentation_domain_retains_symbolic_scale_and_rational_map_guards() {
    let (family, context, roles) = vacuum_fixture(2);
    let reciprocal = context.coefficient_fixture("1/other");
    let routing = MomentumRouting::new(
        vec!["source-k0".into(), "source-k1".into()],
        Vec::new(),
        vec![
            vec![reciprocal, context.zero()],
            vec![context.zero(), context.parameter("other").unwrap()],
        ],
        vec![Vec::new(), Vec::new()],
        Vec::new(),
    );
    let presentation = FamilyPresentation::try_new(
        family,
        roles,
        routing,
        conventions(),
        Some(CommonMassScale::new(context.parameter("m2").unwrap())),
    )
    .unwrap();
    let evidence = presentation.single_scale_vacuum_evidence().unwrap();
    assert_eq!(
        evidence.common_mass_scale_nonzero_numerator(),
        &context.parameter("m2").unwrap().numerator
    );
    let conditions = evidence
        .presentation_domain()
        .conditions()
        .collect::<Vec<_>>();
    assert!(conditions.iter().any(|condition| {
        condition
            .sources()
            .contains(&PresentationConditionSource::CoefficientDenominator(
                PresentationCoefficientLocation::RoutingLoopLinear { row: 0, column: 0 },
            ))
    }));
    assert!(conditions.iter().any(|condition| {
        condition
            .sources()
            .contains(&PresentationConditionSource::CommonMassScaleNumerator)
    }));
}

#[test]
fn role_and_routing_contracts_reject_wrong_maps() {
    let (family, context, mut roles) = vacuum_fixture(2);
    roles.pop();
    let result = FamilyPresentation::try_new(
        family,
        roles,
        identity_routing(&context, 2, 0),
        conventions(),
        None,
    );
    assert!(matches!(
        result,
        Err(FamilyPresentationError::WrongDenominatorRoleCount { .. })
    ));

    let (family, context, mut roles) = vacuum_fixture(2);
    let duplicate = roles[0].id().to_owned();
    roles[1] = DenominatorRole::Auxiliary(AuxiliaryDenominator::new(duplicate));
    let result = FamilyPresentation::try_new(
        family,
        roles,
        identity_routing(&context, 2, 0),
        conventions(),
        None,
    );
    assert!(matches!(
        result,
        Err(FamilyPresentationError::DuplicateDenominatorId { denominator: 1, .. })
    ));

    let (family, context, roles) = vacuum_fixture(2);
    let routing = MomentumRouting::new(
        vec!["source-k0".into(), "source-k1".into()],
        Vec::new(),
        vec![
            vec![context.one(), context.zero()],
            vec![context.zero(), context.integer(2)],
        ],
        vec![Vec::new(), Vec::new()],
        Vec::new(),
    );
    let result = FamilyPresentation::try_new(family, roles, routing, conventions(), None);
    assert!(matches!(
        result,
        Err(FamilyPresentationError::NonUnimodularLoopRouting { .. })
    ));
}
