use super::*;

#[test]
fn presentation_limits_have_exact_and_one_below_boundaries() {
    let exact = FamilyPresentationLimits {
        max_role_and_routing_label_bytes: 11,
        max_coefficient_inputs: 4,
        max_condition_inputs: 5,
        max_nonzero_conditions: 1,
        max_condition_sources: 1,
    };
    assert_eq!(
        one_loop_presentation_with_limits(exact).unwrap().limits(),
        exact
    );

    for (limits, resource, requested, limit) in [
        (
            FamilyPresentationLimits {
                max_role_and_routing_label_bytes: 10,
                ..exact
            },
            "presentation role and routing label bytes",
            11,
            10,
        ),
        (
            FamilyPresentationLimits {
                max_coefficient_inputs: 3,
                ..exact
            },
            "presentation coefficient inputs",
            4,
            3,
        ),
        (
            FamilyPresentationLimits {
                max_condition_inputs: 4,
                ..exact
            },
            "presentation condition inputs",
            5,
            4,
        ),
        (
            FamilyPresentationLimits {
                max_nonzero_conditions: 0,
                ..exact
            },
            "presentation nonzero conditions",
            1,
            0,
        ),
        (
            FamilyPresentationLimits {
                max_condition_sources: 0,
                ..exact
            },
            "presentation condition sources",
            1,
            0,
        ),
    ] {
        assert!(matches!(
            one_loop_presentation_with_limits(limits),
            Err(FamilyPresentationError::ResourceLimit {
                resource: actual_resource,
                requested: actual_requested,
                limit: actual_limit,
            }) if actual_resource == resource
                && actual_requested == requested
                && actual_limit == limit
        ));
    }
}

#[test]
fn constant_guards_are_omitted_and_zero_scale_is_rejected() {
    let context = CoefficientContext::new(["d"]);
    let family = IntegralFamily::new(
        "unit-scale-vacuum",
        vec!["k".into()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        vec![AffineDenominator::new(context.one(), vec![context.one()])],
        Vec::new(),
        vec![context.zero()],
    )
    .unwrap();
    let presentation = FamilyPresentation::try_new(
        family,
        vec![DenominatorRole::Physical(PhysicalPropagator::new(
            "D".to_owned(),
            MomentumCombination::new(vec![context.one()], Vec::new()),
            context.one(),
        ))],
        identity_routing(&context, 1, 0),
        conventions(),
        Some(CommonMassScale::new(context.one())),
    )
    .unwrap();
    assert_eq!(presentation.domain().conditions().count(), 0);

    let (family, context, roles) = vacuum_fixture(1);
    assert!(matches!(
        FamilyPresentation::try_new(
            family,
            roles,
            identity_routing(&context, 1, 0),
            conventions(),
            Some(CommonMassScale::new(context.zero())),
        ),
        Err(FamilyPresentationError::ZeroCommonMassScale)
    ));
}
