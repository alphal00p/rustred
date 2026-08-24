use rustred::{
    AffineDenominator, BaseCoefficientProvenance, BaseKinematicSpecialization, BaseParameterImage,
    BaseSpecializationError, BaseSpecializationGuardProvenance, Coefficient, CoefficientContext,
    CoefficientLocation, FamilyDomainEvaluationStatus, GuardOrigin, IntegralFamily,
};

fn determinant_a_family(context: &CoefficientContext) -> IntegralFamily {
    let a = context.parameter("a").unwrap();
    IntegralFamily::new(
        "determinant-a",
        vec!["k".into()],
        Vec::new(),
        context.clone(),
        context.integer(4),
        vec![AffineDenominator::new(context.zero(), vec![a])],
        Vec::new(),
        vec![context.zero()],
    )
    .unwrap()
}

fn rational_point(
    source: &CoefficientContext,
    assignments: &[(&str, i64)],
) -> BaseKinematicSpecialization {
    let target = CoefficientContext::new(Vec::<String>::new());
    BaseKinematicSpecialization::new(
        source.clone(),
        target.clone(),
        assignments
            .iter()
            .map(|(name, value)| BaseParameterImage::new(*name, target.integer(*value)))
            .collect(),
    )
    .unwrap()
}

#[test]
fn determinant_zero_is_inapplicable_and_nonzero_rational_point_is_applicable() {
    let source = CoefficientContext::new(["a"]);
    let family = determinant_a_family(&source);
    assert_eq!(
        family.domain().basis_determinant(),
        &source.parameter("a").unwrap()
    );

    let at_zero = rational_point(&source, &[("a", 0)]);
    let evaluation = at_zero.evaluate_family_domain(&family).unwrap();
    assert_eq!(
        evaluation.status(),
        FamilyDomainEvaluationStatus::Inapplicable
    );
    assert!(evaluation.guards().is_empty());
    assert_eq!(evaluation.zero_conditions().len(), 1);
    assert_eq!(
        evaluation.zero_conditions()[0].source().location(),
        &CoefficientLocation::BasisDeterminantNumerator
    );
    assert_eq!(
        evaluation.zero_conditions()[0].source().origins(),
        &std::collections::BTreeSet::from([GuardOrigin::FamilyBasisDeterminantNumerator])
    );
    assert!(matches!(
        at_zero.require_family_domain(&family),
        Err(BaseSpecializationError::InapplicableFamilyDomain { zero_conditions })
            if zero_conditions.len() == 1
    ));

    let at_two = rational_point(&source, &[("a", 2)]);
    let evaluation = at_two.require_family_domain(&family).unwrap();
    assert_eq!(
        evaluation.status(),
        FamilyDomainEvaluationStatus::Applicable
    );
    assert!(evaluation.guards().is_empty());
    assert!(evaluation.zero_conditions().is_empty());
}

#[test]
fn mapped_input_denominator_zero_is_rejected_with_its_family_location() {
    let source = CoefficientContext::new(["a"]);
    let family = IntegralFamily::new(
        "input-pole",
        vec!["k".into()],
        Vec::new(),
        source.clone(),
        source.parse("1/(a-1)").unwrap(),
        vec![AffineDenominator::new(source.zero(), vec![source.one()])],
        Vec::new(),
        vec![source.zero()],
    )
    .unwrap();
    let at_pole = rational_point(&source, &[("a", 1)]);

    let evaluation = at_pole.evaluate_family_domain(&family).unwrap();
    assert_eq!(
        evaluation.status(),
        FamilyDomainEvaluationStatus::Inapplicable
    );
    assert_eq!(evaluation.zero_conditions().len(), 1);
    assert_eq!(
        evaluation.zero_conditions()[0].source().location(),
        &CoefficientLocation::Dimension
    );
    assert_eq!(
        evaluation.zero_conditions()[0].source().origins(),
        &std::collections::BTreeSet::from([GuardOrigin::FamilyInputCoefficientDenominator {
            location: CoefficientLocation::Dimension,
        },])
    );
    assert!(matches!(
        at_pole.evaluate_coefficient(
            family.dimension(),
            BaseCoefficientProvenance::Family(CoefficientLocation::Dimension),
        ),
        Err(BaseSpecializationError::MappedCoefficientDenominatorZero {
            source: BaseCoefficientProvenance::Family(CoefficientLocation::Dimension),
        })
    ));
}

#[test]
fn pre_normalization_denominator_guard_survives_cancellation() {
    let source = CoefficientContext::new(["a"]);
    let target = CoefficientContext::new(["u"]);
    let image = target.parse("u+1").unwrap();
    let specialization = BaseKinematicSpecialization::new(
        source.clone(),
        target.clone(),
        vec![BaseParameterImage::new("a", image)],
    )
    .unwrap();

    // Fabricate the mathematically valid but deliberately unnormalized
    // (a^2-1)/(a-1), so the original pole cannot disappear before mapping.
    let coefficient = Coefficient {
        numerator: source.parse("a^2-1").unwrap().numerator,
        denominator: source.parse("a-1").unwrap().numerator,
    };
    let provenance = BaseCoefficientProvenance::named("cancelled-example");
    let mapped = specialization
        .evaluate_coefficient(&coefficient, provenance.clone())
        .unwrap();

    assert_eq!(mapped.value(), &target.parse("u+2").unwrap());
    assert_eq!(mapped.guards().len(), 1);
    assert_eq!(
        mapped.guards()[0].origins(),
        &std::collections::BTreeSet::from([
            BaseSpecializationGuardProvenance::MappedCoefficientDenominator { source: provenance },
        ])
    );
    assert_eq!(
        mapped.guards()[0].polynomial(),
        &target.parameter("u").unwrap().numerator
    );
}

#[test]
fn rational_parameter_image_denominator_is_a_conditional_guard() {
    let source = CoefficientContext::new(["a"]);
    let target = CoefficientContext::new(["u"]);
    let family = determinant_a_family(&source);
    let specialization = BaseKinematicSpecialization::new(
        source,
        target.clone(),
        vec![BaseParameterImage::new("a", target.parse("1/u").unwrap())],
    )
    .unwrap();

    let evaluation = specialization.evaluate_family_domain(&family).unwrap();
    assert_eq!(
        evaluation.status(),
        FamilyDomainEvaluationStatus::Conditional
    );
    assert!(evaluation.zero_conditions().is_empty());
    assert_eq!(evaluation.guards().len(), 1);
    assert_eq!(
        evaluation.guards()[0].origins(),
        &std::collections::BTreeSet::from([
            BaseSpecializationGuardProvenance::ParameterImageDenominator {
                source_parameter_index: 0,
                source_parameter: "a".into(),
            },
        ])
    );
    assert_eq!(
        evaluation.guards()[0].polynomial(),
        &target.parameter("u").unwrap().numerator
    );
    specialization
        .authenticate_guard(&evaluation.guards()[0])
        .unwrap();
    let foreign_target = CoefficientContext::new(["v"]);
    let foreign = BaseKinematicSpecialization::new(
        family.coefficient_context().clone(),
        foreign_target.clone(),
        vec![BaseParameterImage::new(
            "a",
            foreign_target.parse("1/v").unwrap(),
        )],
    )
    .unwrap();
    assert!(matches!(
        foreign.authenticate_guard(&evaluation.guards()[0]),
        Err(BaseSpecializationError::ForeignTargetGuard)
    ));
}

#[test]
fn coincident_input_and_determinant_locus_is_reported_once_with_both_origins() {
    let source = CoefficientContext::new(["a"]);
    let family = IntegralFamily::new(
        "coincident-domain-locus",
        vec!["k".into()],
        Vec::new(),
        source.clone(),
        source.parse("1/a").unwrap(),
        vec![AffineDenominator::new(
            source.zero(),
            vec![source.parameter("a").unwrap()],
        )],
        Vec::new(),
        vec![source.zero()],
    )
    .unwrap();
    assert_eq!(family.domain().conditions().count(), 1);

    let evaluation = rational_point(&source, &[("a", 0)])
        .evaluate_family_domain(&family)
        .unwrap();
    assert_eq!(
        evaluation.status(),
        FamilyDomainEvaluationStatus::Inapplicable
    );
    assert_eq!(evaluation.zero_conditions().len(), 1);
    let source = evaluation.zero_conditions()[0].source();
    assert!(
        source
            .origins()
            .contains(&GuardOrigin::FamilyBasisDeterminantNumerator)
    );
    assert!(
        source
            .origins()
            .contains(&GuardOrigin::FamilyInputCoefficientDenominator {
                location: CoefficientLocation::Dimension,
            })
    );

    let target = CoefficientContext::new(["u"]);
    let symbolic = BaseKinematicSpecialization::new(
        family.coefficient_context().clone(),
        target.clone(),
        vec![BaseParameterImage::new("a", target.parameter("u").unwrap())],
    )
    .unwrap();
    let conditional = symbolic.evaluate_family_domain(&family).unwrap();
    assert_eq!(
        conditional.status(),
        FamilyDomainEvaluationStatus::Conditional
    );
    assert_eq!(conditional.guards().len(), 1);
    let guard_origins = conditional.guards()[0].origins();
    assert!(
        guard_origins.contains(&BaseSpecializationGuardProvenance::FamilyDomainOrigin {
            origin: GuardOrigin::FamilyBasisDeterminantNumerator,
        })
    );
    assert!(
        guard_origins.contains(&BaseSpecializationGuardProvenance::FamilyDomainOrigin {
            origin: GuardOrigin::FamilyInputCoefficientDenominator {
                location: CoefficientLocation::Dimension,
            },
        })
    );
}

#[test]
fn distinct_source_conditions_that_map_to_zero_merge_all_origins() {
    let source = CoefficientContext::new(["a", "b"]);
    let family = IntegralFamily::new(
        "mapped-coincident-zero",
        vec!["k".into()],
        Vec::new(),
        source.clone(),
        source.parse("1/a").unwrap(),
        vec![AffineDenominator::new(source.zero(), vec![source.one()])],
        Vec::new(),
        vec![source.parse("1/b").unwrap()],
    )
    .unwrap();
    assert_eq!(family.domain().conditions().count(), 3);

    let evaluation = rational_point(&source, &[("a", 0), ("b", 0)])
        .evaluate_family_domain(&family)
        .unwrap();
    assert_eq!(
        evaluation.status(),
        FamilyDomainEvaluationStatus::Inapplicable
    );
    assert_eq!(evaluation.zero_conditions().len(), 1);
    let origins = evaluation.zero_conditions()[0].source().origins();
    assert!(
        origins.contains(&GuardOrigin::FamilyInputCoefficientDenominator {
            location: CoefficientLocation::Dimension,
        })
    );
    assert!(
        origins.contains(&GuardOrigin::FamilyInputCoefficientDenominator {
            location: CoefficientLocation::PowerShift { denominator: 0 },
        })
    );

    let target = CoefficientContext::new(["u"]);
    let u = target.parameter("u").unwrap();
    let symbolic = BaseKinematicSpecialization::new(
        family.coefficient_context().clone(),
        target,
        vec![
            BaseParameterImage::new("a", u.clone()),
            BaseParameterImage::new("b", u),
        ],
    )
    .unwrap();
    let conditional = symbolic.evaluate_family_domain(&family).unwrap();
    assert_eq!(
        conditional.status(),
        FamilyDomainEvaluationStatus::Conditional
    );
    assert_eq!(conditional.guards().len(), 1);
    let origins = conditional.guards()[0].origins();
    assert!(
        origins.contains(&BaseSpecializationGuardProvenance::FamilyDomainOrigin {
            origin: GuardOrigin::FamilyInputCoefficientDenominator {
                location: CoefficientLocation::Dimension,
            },
        })
    );
    assert!(
        origins.contains(&BaseSpecializationGuardProvenance::FamilyDomainOrigin {
            origin: GuardOrigin::FamilyInputCoefficientDenominator {
                location: CoefficientLocation::PowerShift { denominator: 0 },
            },
        })
    );
}

#[test]
fn singular_external_gram_is_allowed_and_is_not_a_domain_guard() {
    let source = CoefficientContext::new(["g"]);
    let family = IntegralFamily::new(
        "null-external",
        vec!["k".into()],
        vec!["p".into()],
        source.clone(),
        source.integer(4),
        vec![
            AffineDenominator::new(source.zero(), vec![source.one(), source.zero()]),
            AffineDenominator::new(source.zero(), vec![source.zero(), source.one()]),
        ],
        vec![vec![source.parameter("g").unwrap()]],
        vec![source.zero(), source.zero()],
    )
    .unwrap();
    let null_point = rational_point(&source, &[("g", 0)]);

    let evaluation = null_point.require_family_domain(&family).unwrap();
    assert_eq!(
        evaluation.status(),
        FamilyDomainEvaluationStatus::Applicable
    );
    assert!(evaluation.guards().is_empty());
}

#[test]
fn source_parameter_manifest_is_named_and_order_authenticated() {
    let source = CoefficientContext::new(["a", "b"]);
    let target = CoefficientContext::new(Vec::<String>::new());
    let result = BaseKinematicSpecialization::new(
        source,
        target.clone(),
        vec![
            BaseParameterImage::new("b", target.integer(1)),
            BaseParameterImage::new("a", target.integer(2)),
        ],
    );
    assert!(matches!(
        result,
        Err(BaseSpecializationError::SourceParameterMismatch {
            position: 0,
            expected,
            actual,
        }) if expected == "a" && actual == "b"
    ));
}
