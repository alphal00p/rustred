use std::sync::Arc;

use rustred::{
    AffineDenominator, CoefficientContext, GeneratedCylindricalResidualStartCertificate,
    GeneratedCylindricalResidualStartLimits, GeneratedCylindricalRowSystemCertificate,
    GeneratedCylindricalRowSystemError, GeneratedCylindricalRowSystemLimits,
    GeneratedCylindricalSourceRowOutcome, GeneratedCylindricalStartCompleteness,
    GeneratedSectorDiscoveryCompiler, GeneratedSectorDiscoveryLimits,
    GeneratedSectorLiveLeafQueueCertificate, GeneratedSectorLiveLeafQueueCompiler,
    GeneratedSectorLiveLeafQueueLimits, IntegralFamily, IntegralOrderingPolicy,
    ParametricIbpGenerator, ParametricRelationError, SectorMask, SymbolicPolynomialPredicateKind,
};

fn tadpole(name: &str, power_shift: bool) -> IntegralFamily {
    let coefficients = if power_shift {
        CoefficientContext::new(["d", "m2", "nu"])
    } else {
        CoefficientContext::new(["d", "m2"])
    };
    let shifts = if power_shift {
        vec![coefficients.parameter("nu").unwrap()]
    } else {
        vec![coefficients.zero()]
    };
    IntegralFamily::new(
        name,
        vec!["k".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![AffineDenominator::new(
            coefficients.parse("-m2").unwrap(),
            vec![coefficients.one()],
        )],
        Vec::new(),
        shifts,
    )
    .unwrap()
}

fn residual_queue(
    family: &IntegralFamily,
) -> (
    rustred::ParametricCoefficientContext,
    Arc<GeneratedSectorLiveLeafQueueCertificate>,
) {
    let context = ParametricIbpGenerator::try_new(family)
        .unwrap()
        .context()
        .clone();
    let mut discovery_limits = GeneratedSectorDiscoveryLimits::default();
    discovery_limits.adaptive.max_search_depth = 0;
    let discovery = GeneratedSectorDiscoveryCompiler::compile(
        family,
        &context,
        SectorMask::try_new([true]).unwrap(),
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        discovery_limits,
    )
    .unwrap();
    let mut queue_limits = GeneratedSectorLiveLeafQueueLimits::default();
    queue_limits.translation_radius = 0;
    queue_limits.max_translation_points = 1;
    let queue =
        GeneratedSectorLiveLeafQueueCompiler::compile(family, &context, &discovery, queue_limits)
            .unwrap();
    (context, Arc::new(queue))
}

fn start_with_pending_equalities(
    family: &IntegralFamily,
    context: &rustred::ParametricCoefficientContext,
    queue: Arc<GeneratedSectorLiveLeafQueueCertificate>,
    pending: bool,
    depth: usize,
) -> Arc<GeneratedCylindricalResidualStartCertificate> {
    let item = queue
        .work_items()
        .iter()
        .find(|item| {
            item.extraction()
                .unresolved_predicates()
                .iter()
                .any(|predicate| predicate.kind() == SymbolicPolynomialPredicateKind::EqualZero)
                == pending
        })
        .unwrap_or_else(|| {
            panic!(
                "fixture has no residual item with pending equality status {pending}; unresolved predicates per item: {:?}",
                queue
                    .work_items()
                    .iter()
                    .map(|item| item.extraction().unresolved_predicates().len())
                    .collect::<Vec<_>>()
            )
        });
    let item_ordinal = item.ordinal();
    Arc::new(
        GeneratedCylindricalResidualStartCertificate::compile(
            family,
            context,
            queue,
            item_ordinal,
            depth,
            GeneratedCylindricalResidualStartLimits::default(),
        )
        .unwrap(),
    )
}

#[test]
fn independent_rows_are_point_major_replayable_and_bounded_before_row_work() {
    let family = tadpole("generated-cylindrical-row-system-public", false);
    let (context, queue) = residual_queue(&family);
    let start = start_with_pending_equalities(&family, &context, queue, false, 1);
    assert_eq!(
        start.completeness(),
        &GeneratedCylindricalStartCompleteness::IndependentIntegerCylinder
    );

    let certificate = GeneratedCylindricalRowSystemCertificate::compile(
        &family,
        &context,
        start.clone(),
        GeneratedCylindricalRowSystemLimits::default(),
    )
    .unwrap();
    certificate.replay(&family, &context).unwrap();

    let source_rows = start.source_queue().discovery().row_span().rows().len();
    let prepare_points = start.schedule().stats().retained_points();
    let stats = certificate.stats();
    assert_eq!(stats.source_rows(), source_rows);
    assert_eq!(stats.prepare_points(), prepare_points);
    assert_eq!(stats.expanded_rows(), source_rows * prepare_points);
    assert_eq!(certificate.witnesses().len(), stats.expanded_rows());
    assert_eq!(
        stats.retained_rows() + stats.unsatisfiable_rows(),
        stats.expanded_rows()
    );

    let mut expected_expanded = 0usize;
    let mut expected_retained = 0usize;
    for (layer_ordinal, layer) in start.schedule().layers().iter().enumerate() {
        for prepare_point_ordinal in 0..layer.ordered_translations().len() {
            for source_row_ordinal in 0..source_rows {
                let witness = &certificate.witnesses()[expected_expanded];
                assert_eq!(witness.expanded_ordinal(), expected_expanded);
                assert_eq!(witness.layer_ordinal(), layer_ordinal);
                assert_eq!(witness.depth(), layer.depth());
                assert_eq!(witness.prepare_point_ordinal(), prepare_point_ordinal);
                assert_eq!(witness.source_row_ordinal(), source_row_ordinal);
                if let GeneratedCylindricalSourceRowOutcome::Retained {
                    retained_row_ordinal,
                    ..
                } = witness.outcome()
                {
                    assert_eq!(*retained_row_ordinal, expected_retained);
                    expected_retained += 1;
                }
                expected_expanded += 1;
            }
        }
    }
    assert_eq!(expected_retained, stats.retained_rows());

    let mut limits = GeneratedCylindricalRowSystemLimits::default();
    limits.max_expanded_rows = stats.expanded_rows() - 1;
    assert!(matches!(
        GeneratedCylindricalRowSystemCertificate::compile(&family, &context, start.clone(), limits,),
        Err(GeneratedCylindricalRowSystemError::ResourceLimit {
            resource: "expanded rows",
            ..
        })
    ));

    limits = GeneratedCylindricalRowSystemLimits::default();
    limits.max_derived_row_label_bytes = 0;
    assert!(matches!(
        GeneratedCylindricalRowSystemCertificate::compile(
            &family,
            &context,
            start.clone(),
            limits,
        ),
        Err(GeneratedCylindricalRowSystemError::ResourceLimit {
            resource: "derived row label bytes",
            requested,
            limit: 0,
        }) if requested > 0
    ));

    limits = GeneratedCylindricalRowSystemLimits::default();
    limits.max_total_translated_terms = 0;
    assert!(matches!(
        GeneratedCylindricalRowSystemCertificate::compile(
            &family,
            &context,
            start.clone(),
            limits,
        ),
        Err(GeneratedCylindricalRowSystemError::ResourceLimit {
            resource: "translated terms",
            requested,
            limit: 0,
        }) if requested > 0
    ));

    limits = GeneratedCylindricalRowSystemLimits::default();
    limits.max_total_specialization_source_terms = 0;
    assert!(matches!(
        GeneratedCylindricalRowSystemCertificate::compile(&family, &context, start, limits),
        Err(GeneratedCylindricalRowSystemError::Relation(
            ParametricRelationError::ResourceLimit {
                resource: "partial relation source terms",
                requested,
                limit: 0,
            }
        )) if requested > 0
    ));
}

#[test]
fn dependent_symbolic_start_is_rejected_before_integer_cylinder_limits() {
    let family = tadpole("generated-cylindrical-row-system-dependent", true);
    let (context, queue) = residual_queue(&family);
    let start = start_with_pending_equalities(&family, &context, queue, true, 0);
    let pending = start
        .completeness()
        .pending_equality_predicate_ordinals()
        .len();
    assert!(pending > 0);

    let mut limits = GeneratedCylindricalRowSystemLimits::default();
    limits.max_source_rows = 0;
    assert_eq!(
        GeneratedCylindricalRowSystemCertificate::compile(&family, &context, start, limits)
            .unwrap_err(),
        GeneratedCylindricalRowSystemError::IncompleteDependentSymbolicStart {
            unresolved_equality_predicates: pending,
        }
    );
}
