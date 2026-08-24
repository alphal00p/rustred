use std::sync::Arc;

use rustred::{
    AffineDenominator, CoefficientContext, GeneratedCylindricalResidualStartCertificate,
    GeneratedCylindricalResidualStartError, GeneratedCylindricalResidualStartLimits,
    GeneratedCylindricalStartCompleteness, GeneratedFamilyRuleSystemCompiler,
    GeneratedFamilyRuleSystemConfig, GeneratedFamilyRuleSystemLimits, GeneratedFamilySectorStatus,
    IntegralFamily, IntegralOrderingPolicy, ParametricIbpGenerator, PowerShiftPolicy,
    SectorRestrictions, SymbolicPolynomialPredicateKind,
};

fn massive_tadpole() -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    IntegralFamily::new(
        "generated-cylindrical-start-tadpole",
        vec!["k".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![AffineDenominator::new(
            coefficients.parse("-m2").unwrap(),
            vec![coefficients.one()],
        )],
        Vec::new(),
        vec![coefficients.zero()],
    )
    .unwrap()
}

#[test]
fn live_residual_item_compiles_without_a_concrete_corner_and_replays() {
    let family = massive_tadpole();
    let context = ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .context()
        .clone();
    let mut family_limits = GeneratedFamilyRuleSystemLimits::default();
    family_limits.discovery.adaptive.max_search_depth = 0;
    family_limits.live_leaf_queue.translation_radius = 0;
    family_limits.live_leaf_queue.max_translation_points = 1;
    let base = GeneratedFamilyRuleSystemCompiler::compile(
        &family,
        &context,
        SectorRestrictions::unrestricted(family.denominator_count()).unwrap(),
        PowerShiftPolicy::FormalGeneric,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        GeneratedFamilyRuleSystemConfig::default(),
        family_limits,
    )
    .unwrap();

    let queue = base
        .solve_order()
        .iter()
        .find_map(|sector| match base.status(sector).unwrap() {
            GeneratedFamilySectorStatus::Unresolved {
                live_leaf_queue, ..
            } if !live_leaf_queue.work_items().is_empty() => {
                Some(Arc::new(live_leaf_queue.clone()))
            }
            _ => None,
        })
        .expect("the bounded generated tadpole system must retain a residual work item");
    let item = &queue.work_items()[0];
    let item_ordinal = item.ordinal();
    let item_source_case = item.source_case();
    let item_assignment = item.extraction().assignment().clone();
    let item_pending = item
        .extraction()
        .unresolved_predicates()
        .iter()
        .filter(|predicate| predicate.kind() == SymbolicPolynomialPredicateKind::EqualZero)
        .map(|predicate| predicate.predicate_ordinal())
        .collect::<Vec<_>>();
    let certificate = GeneratedCylindricalResidualStartCertificate::compile(
        &family,
        &context,
        queue.clone(),
        item_ordinal,
        2,
        GeneratedCylindricalResidualStartLimits::default(),
    )
    .unwrap();
    certificate.replay(&family, &context).unwrap();

    assert_eq!(certificate.source_case(), item_source_case);
    assert_eq!(certificate.assignment(), &item_assignment);
    assert_eq!(certificate.schedule().layers().len(), 3);
    assert_eq!(certificate.schedule().layers()[0].depth(), 0);
    assert_eq!(
        certificate.schedule().ordering().free_positions().len(),
        family.denominator_count() - item_assignment.entries().len()
    );
    match certificate.completeness() {
        GeneratedCylindricalStartCompleteness::IndependentIntegerCylinder => {
            assert!(item_pending.is_empty())
        }
        GeneratedCylindricalStartCompleteness::DependentSymbolicStartPending {
            unresolved_equality_predicate_ordinals,
        } => assert_eq!(
            unresolved_equality_predicate_ordinals.as_ref(),
            item_pending
        ),
    }

    // The ordinal limit is checked before queue lookup.  Exercise a genuinely
    // nonzero request even though this minimal queue happens to retain only
    // ordinal zero.
    let limits = GeneratedCylindricalResidualStartLimits {
        max_work_item_ordinal: 0,
        ..GeneratedCylindricalResidualStartLimits::default()
    };
    assert!(matches!(
        GeneratedCylindricalResidualStartCertificate::compile(
            &family, &context, queue, 1, 0, limits,
        ),
        Err(
            GeneratedCylindricalResidualStartError::WorkItemOrdinalLimit {
                requested: 1,
                limit: 0,
            }
        )
    ));
}
