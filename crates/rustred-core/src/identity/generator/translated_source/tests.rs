use std::sync::Arc;

use crate::algebra::{
    CoefficientContext, ExactAlgebraError, IndexedAlgebraError, IndexedAlgebraLimits,
    IndexedCoefficientContext,
};
use crate::family::{AffineDenominator, IntegralFamily};
use crate::identity::{
    IdentityConditionError, IdentityConditionSource, ParametricRelationError, RowId,
};

use super::super::model::{CompletedIbpSourceRows, ParametricIbpGenerator};
use super::construction::{add_condition_source_entries, retained_condition_source_entry_bound};
use super::{
    IntegralShift, TranslatedSourceError, TranslatedSourceLimits, TranslatedSourceRequest,
};

fn equal_mass_sunset(name: &str) -> (CoefficientContext, IntegralFamily) {
    let base = CoefficientContext::new(["d", "s"]);
    let zero = base.zero();
    let one = base.one();
    let minus_s = base
        .try_neg(&base.parameter("s").unwrap(), Default::default())
        .unwrap();
    let family = IntegralFamily::new(
        name,
        vec!["k1".into(), "k2".into()],
        Vec::new(),
        base.clone(),
        base.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(
                minus_s.clone(),
                vec![one.clone(), zero.clone(), zero.clone()],
            ),
            AffineDenominator::new(
                minus_s.clone(),
                vec![zero.clone(), zero.clone(), one.clone()],
            ),
            AffineDenominator::new(minus_s, vec![one.clone(), base.integer(2), one]),
        ],
        Vec::new(),
        vec![zero.clone(), zero.clone(), zero],
    )
    .unwrap();
    (base, family)
}

fn guarded_tadpole(name: &str) -> IntegralFamily {
    let base = CoefficientContext::new(["d", "x"]);
    let reciprocal = base
        .try_div(
            &base.one(),
            &base.parameter("x").unwrap(),
            Default::default(),
        )
        .unwrap();
    IntegralFamily::new(
        name,
        vec!["k".into()],
        Vec::new(),
        base.clone(),
        base.parameter("d").unwrap(),
        vec![AffineDenominator::new(base.integer(-1), vec![base.one()])],
        Vec::new(),
        vec![reciprocal],
    )
    .unwrap()
}

fn one_loop_one_external(name: &str) -> IntegralFamily {
    let base = CoefficientContext::new(["d", "s"]);
    IntegralFamily::new(
        name,
        vec!["k".into()],
        vec!["p".into()],
        base.clone(),
        base.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(base.integer(-1), vec![base.one(), base.zero()]),
            AffineDenominator::new(base.zero(), vec![base.zero(), base.one()]),
        ],
        vec![vec![base.parameter("s").unwrap()]],
        vec![base.zero(), base.zero()],
    )
    .unwrap()
}

fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let batch = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..batch.len())
        .map(|ordinal| batch.generate(ordinal))
        .collect();
    batch.complete(rows).unwrap()
}

fn complete_external(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let batch = generator.prepare_external_ibp_sources().unwrap();
    let rows = (0..batch.len())
        .map(|ordinal| batch.generate(ordinal))
        .collect();
    batch.complete(rows).unwrap()
}

#[derive(Clone, Copy, Debug)]
enum ZeroTemplateDiagnostic {
    Aggregate(&'static str),
    Indexed,
    Exact,
    Condition,
}

fn zero_template_coefficient_error(error: ParametricRelationError) -> IndexedAlgebraError {
    match error {
        ParametricRelationError::Coefficient(error)
        | ParametricRelationError::IdentityCondition(IdentityConditionError::Coefficient(error)) => {
            error
        }
        other => panic!("expected native coefficient diagnostic, got {other:?}"),
    }
}

fn assert_zero_template_diagnostic(error: TranslatedSourceError, expected: ZeroTemplateDiagnostic) {
    match (expected, error) {
        (
            ZeroTemplateDiagnostic::Aggregate(expected),
            TranslatedSourceError::ResourceLimit {
                resource,
                requested,
                limit: 0,
            },
        ) => {
            assert_eq!(resource, expected);
            assert!(requested > 0);
        }
        (
            ZeroTemplateDiagnostic::Indexed,
            TranslatedSourceError::RelationTranslation { error, .. }
            | TranslatedSourceError::RequestTranslation { error, .. },
        ) => assert!(matches!(zero_template_coefficient_error(error),
            IndexedAlgebraError::ResourceLimit { requested, limit: 0, .. } if requested > 0)),
        (
            ZeroTemplateDiagnostic::Exact,
            TranslatedSourceError::RelationTranslation { error, .. }
            | TranslatedSourceError::RequestTranslation { error, .. },
        ) => assert!(matches!(zero_template_coefficient_error(error),
            IndexedAlgebraError::ExactAlgebra(ExactAlgebraError::ResourceLimit { requested, limit: 0, .. }) if requested > 0)),
        (
            ZeroTemplateDiagnostic::Condition,
            TranslatedSourceError::RelationTranslation { error, .. }
            | TranslatedSourceError::RequestTranslation { error, .. },
        ) => assert!(matches!(error,
            ParametricRelationError::IdentityCondition(IdentityConditionError::ResourceLimit { resource: "identity condition sources", requested, limit: 0 }) if requested > 0)),
        (expected, actual) => {
            panic!("expected {expected:?} zero-template diagnostic, got {actual:?}")
        }
    }
}

fn assert_independent_zero_translation_template(
    set_zero: fn(&mut TranslatedSourceLimits),
    rectangular_diagnostic: Option<ZeroTemplateDiagnostic>,
    selected_diagnostic: ZeroTemplateDiagnostic,
) {
    let family = guarded_tadpole("translated-source-independent-zero-template");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let offsets = [
        IntegralShift::try_new([-1]).unwrap(),
        IntegralShift::try_new([0]).unwrap(),
    ];
    let requests = offsets
        .iter()
        .map(|offset| TranslatedSourceRequest::new(0, offset.clone()))
        .collect::<Vec<_>>();
    let mut limits = TranslatedSourceLimits::default();
    set_zero(&mut limits);
    let expected = generator
        .translate_completed_source_rows(&completed, offsets.clone(), Default::default())
        .unwrap();
    assert!(!expected.sources()[0].nonzero_conditions().is_empty());
    let ordinary = generator
        .translate_completed_source_rows(&completed, offsets.clone(), limits)
        .unwrap();
    assert_eq!(ordinary, expected);
    let expected_selected = generator
        .translate_selected_completed_source_rows(&completed, requests.clone(), Default::default())
        .unwrap();
    let ordinary_selected = generator
        .translate_selected_completed_source_rows(&completed, requests.clone(), limits)
        .unwrap();
    assert_eq!(ordinary_selected, expected_selected);
    assert_eq!(ordinary_selected.sources(), ordinary.sources());
    let diagnostic =
        generator.translate_completed_source_rows(&completed, offsets, limits.with_diagnostics());
    if let Some(expected_diagnostic) = rectangular_diagnostic {
        assert_zero_template_diagnostic(diagnostic.unwrap_err(), expected_diagnostic);
    } else {
        // This template controls selected requests; rectangular translation has none.
        assert_eq!(diagnostic.unwrap(), expected);
    }
    assert_zero_template_diagnostic(
        generator
            .translate_selected_completed_source_rows(
                &completed,
                requests,
                limits.with_diagnostics(),
            )
            .unwrap_err(),
        selected_diagnostic,
    );
}

macro_rules! independent_zero_translation_template {
    ($name:ident, $set_zero:expr, $rectangular:expr, $selected:expr) => {
        #[test]
        fn $name() {
            assert_independent_zero_translation_template($set_zero, $rectangular, $selected);
        }
    };
}

independent_zero_translation_template!(
    ordinary_zero_requested_offsets_preserves_translation,
    |limits| limits.max_requested_offsets = 0,
    Some(ZeroTemplateDiagnostic::Aggregate(
        "requested translated-source offsets"
    )),
    ZeroTemplateDiagnostic::Aggregate("canonical selected translation offsets")
);
independent_zero_translation_template!(
    ordinary_zero_selected_requests_preserves_translation,
    |limits| limits.max_requested_source_translations = 0,
    None,
    ZeroTemplateDiagnostic::Aggregate("requested selected source translations")
);
independent_zero_translation_template!(
    ordinary_zero_translated_sources_preserves_translation,
    |limits| limits.max_translated_sources = 0,
    Some(ZeroTemplateDiagnostic::Aggregate("translated source rows")),
    ZeroTemplateDiagnostic::Aggregate("translated source rows")
);
independent_zero_translation_template!(
    ordinary_zero_translated_terms_preserves_translation,
    |limits| limits.max_translated_term_entries = 0,
    Some(ZeroTemplateDiagnostic::Aggregate(
        "translated source term entries"
    )),
    ZeroTemplateDiagnostic::Aggregate("translated source term entries")
);
independent_zero_translation_template!(
    ordinary_zero_translated_conditions_preserves_translation,
    |limits| limits.max_translated_condition_entries = 0,
    Some(ZeroTemplateDiagnostic::Aggregate(
        "translated source condition entries"
    )),
    ZeroTemplateDiagnostic::Aggregate("translated source condition entries")
);
independent_zero_translation_template!(
    ordinary_zero_condition_provenance_preserves_translation,
    |limits| limits.max_retained_condition_source_entries = 0,
    Some(ZeroTemplateDiagnostic::Aggregate(
        "translated-source retained condition-source entries"
    )),
    ZeroTemplateDiagnostic::Aggregate("translated-source retained condition-source entries")
);
independent_zero_translation_template!(
    ordinary_zero_coordinate_cells_preserves_translation,
    |limits| limits.max_retained_index_coordinate_cells = 0,
    Some(ZeroTemplateDiagnostic::Aggregate(
        "translated-source retained index-coordinate cells"
    )),
    ZeroTemplateDiagnostic::Aggregate("translated-source retained index-coordinate cells")
);
independent_zero_translation_template!(
    ordinary_zero_indexed_translation_work_preserves_translation,
    |limits| limits.relation.arithmetic.max_specialization_integer_bits = 0,
    Some(ZeroTemplateDiagnostic::Indexed),
    ZeroTemplateDiagnostic::Indexed
);
independent_zero_translation_template!(
    ordinary_zero_exact_term_template_preserves_translation,
    |limits| limits
        .relation
        .arithmetic
        .exact_algebra
        .max_polynomial_terms = 0,
    Some(ZeroTemplateDiagnostic::Exact),
    ZeroTemplateDiagnostic::Exact
);
independent_zero_translation_template!(
    ordinary_zero_condition_sources_preserves_translation,
    |limits| limits.relation.identity_conditions.max_sources = 0,
    Some(ZeroTemplateDiagnostic::Condition),
    ZeroTemplateDiagnostic::Condition
);

#[test]
fn ordinary_zero_translation_templates_preserve_sources_guards_and_selected_parity() {
    let family = guarded_tadpole("translated-source-zero-templates");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let offsets = [
        IntegralShift::try_new([-1]).unwrap(),
        IntegralShift::try_new([0]).unwrap(),
    ];
    let mut limits = TranslatedSourceLimits {
        diagnostics_enabled: false,
        max_requested_offsets: 0,
        max_requested_source_translations: 0,
        max_translated_sources: 0,
        max_translated_term_entries: 0,
        max_translated_condition_entries: 0,
        max_retained_condition_source_entries: 0,
        max_retained_index_coordinate_cells: 0,
        ..Default::default()
    };
    limits.relation.arithmetic.max_specialization_integer_bits = 0;
    limits
        .relation
        .arithmetic
        .exact_algebra
        .max_polynomial_terms = 0;
    limits.relation.identity_conditions.max_sources = 0;
    let expected = generator
        .translate_completed_source_rows(&completed, offsets.clone(), Default::default())
        .unwrap();
    assert!(!expected.sources()[0].nonzero_conditions().is_empty());
    let ordinary = generator
        .translate_completed_source_rows(&completed, offsets.clone(), limits)
        .unwrap();
    assert_eq!(ordinary, expected);
    let requests = offsets
        .iter()
        .map(|offset| TranslatedSourceRequest::new(0, offset.clone()))
        .collect::<Vec<_>>();
    let selected = generator
        .translate_selected_completed_source_rows(&completed, requests.clone(), limits)
        .unwrap();
    let expected_selected = generator
        .translate_selected_completed_source_rows(&completed, requests.clone(), Default::default())
        .unwrap();
    assert_eq!(selected, expected_selected);
    assert_eq!(selected.sources(), ordinary.sources());
    assert!(matches!(
        generator.translate_completed_source_rows(&completed, offsets, limits.with_diagnostics()),
        Err(TranslatedSourceError::ResourceLimit {
            resource: "requested translated-source offsets",
            requested: 1,
            limit: 0,
        })
    ));
    assert!(matches!(
        generator.translate_selected_completed_source_rows(
            &completed,
            requests,
            limits.with_diagnostics()
        ),
        Err(TranslatedSourceError::ResourceLimit {
            resource: "requested selected source translations",
            requested: 1,
            limit: 0,
        })
    ));

    let shift = IntegralShift::try_new(vec![0; 4_097]).unwrap();
    assert_eq!(shift.len(), 4_097);
    assert_eq!(
        IntegralShift::try_new_with_component_limit(vec![0; 4_097], 4_096),
        Err(TranslatedSourceError::ResourceLimit {
            resource: "integral-shift components",
            requested: 4_097,
            limit: 4_096,
        })
    );
}

#[test]
fn translated_source_resource_refusal_preserves_original_cause_and_healthy_reuse() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use symbolica::utils::resources::{ResourceBudget, ResourceLimitError, ResourceLimits};

    const LIMIT: u64 = 1024 * 1024 * 1024;
    let family = guarded_tadpole("translated-source-resource-reuse");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let offsets = [
        IntegralShift::try_new([-1]).unwrap(),
        IntegralShift::try_new([0]).unwrap(),
    ];
    let build = || {
        generator.translate_completed_source_rows(&completed, offsets.clone(), Default::default())
    };
    let expected = build().unwrap();
    let polls = Arc::new(AtomicUsize::new(0));
    let monitor_polls = Arc::clone(&polls);
    ResourceBudget::with_memory_monitor(
        ResourceLimits {
            max_process_rss_bytes: Some(LIMIT),
            ..Default::default()
        },
        move || {
            monitor_polls.fetch_add(1, Ordering::Relaxed);
            Ok(0)
        },
    )
    .run(build)
    .unwrap();
    let checks = polls.load(Ordering::Relaxed);
    assert!(checks > offsets.len());
    let refusal_polls = AtomicUsize::new(0);
    let error = ResourceBudget::with_memory_monitor(
        ResourceLimits {
            max_process_rss_bytes: Some(LIMIT),
            ..Default::default()
        },
        move || {
            Ok(
                if refusal_polls.fetch_add(1, Ordering::Relaxed) + 1 >= checks {
                    LIMIT + 1
                } else {
                    0
                },
            )
        },
    )
    .run(build)
    .unwrap_err();
    assert!(matches!(crate::reduction::resource_limit_error(&error),
        Some(ResourceLimitError::Memory { rss_bytes, additional_bytes: 0, limit_bytes })
            if *rss_bytes == LIMIT + 1 && *limit_bytes == LIMIT));
    assert_eq!(build().unwrap(), expected);

    let selected_build = || {
        generator.translate_selected_completed_source_rows(
            &completed,
            offsets
                .iter()
                .map(|offset| TranslatedSourceRequest::new(0, offset.clone())),
            Default::default(),
        )
    };
    let expected_selected = selected_build().unwrap();
    let error = ResourceBudget::new(ResourceLimits {
        max_elapsed: Some(Duration::ZERO),
        ..Default::default()
    })
    .run(selected_build)
    .unwrap_err();
    assert!(matches!(
        crate::reduction::resource_limit_error(&error),
        Some(ResourceLimitError::Elapsed { .. })
    ));
    assert_eq!(selected_build().unwrap(), expected_selected);
    assert_eq!(expected_selected.sources(), expected.sources());
}

#[test]
fn rectangular_and_selected_batches_preserve_their_sealed_source_layout() {
    let family = one_loop_one_external("translated-source-layout-seal");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let zero = || IntegralShift::try_new([0, 0]).unwrap();

    let ordinary = complete_ordinary(&generator);
    let ordinary_rectangular = generator
        .translate_completed_source_rows(&ordinary, [zero()], TranslatedSourceLimits::default())
        .unwrap();
    let ordinary_selected = generator
        .translate_selected_completed_source_rows(
            &ordinary,
            [TranslatedSourceRequest::new(0, zero())],
            TranslatedSourceLimits::default(),
        )
        .unwrap();
    assert!(ordinary_rectangular.is_complete_ordinary());
    assert!(ordinary_selected.is_complete_ordinary());
    assert_eq!(
        ordinary_rectangular.source_layout_name(),
        "ordinary IBP source"
    );
    assert_eq!(
        ordinary_selected.source_layout_name(),
        "ordinary IBP source"
    );

    let external = complete_external(&generator);
    let external_rectangular = generator
        .translate_completed_source_rows(&external, [zero()], TranslatedSourceLimits::default())
        .unwrap();
    let external_selected = generator
        .translate_selected_completed_source_rows(
            &external,
            [TranslatedSourceRequest::new(0, zero())],
            TranslatedSourceLimits::default(),
        )
        .unwrap();
    assert!(!external_rectangular.is_complete_ordinary());
    assert!(!external_selected.is_complete_ordinary());
    assert_eq!(
        external_rectangular.source_layout_name(),
        "external-contraction IBP source"
    );
    assert_eq!(
        external_selected.source_layout_name(),
        "external-contraction IBP source"
    );
}

fn exact_translation_replays(
    context: &IndexedCoefficientContext,
    source: &super::super::super::relation::ParametricRelation,
    translated: &super::TranslatedSource,
    offset: &[i64],
) {
    assert_eq!(translated.row_id(), source.row_id());
    assert_eq!(translated.terms().len(), source.terms().len());
    for (source_shift, source_coefficient) in source.terms() {
        let expected_shift = source_shift
            .values()
            .iter()
            .zip(offset)
            .map(|(left, right)| left.checked_add(*right).unwrap())
            .collect::<Vec<_>>();
        let translated_coefficient = translated
            .terms()
            .iter()
            .find_map(|(shift, coefficient)| {
                (shift.values() == expected_shift).then_some(coefficient)
            })
            .unwrap();
        let expected_coefficient = context
            .translate(source_coefficient, offset, IndexedAlgebraLimits::default())
            .unwrap();
        assert_eq!(translated_coefficient, &expected_coefficient);

        let inverse = offset.iter().map(|value| -*value).collect::<Vec<_>>();
        let replayed = context
            .translate(
                translated_coefficient,
                &inverse,
                IndexedAlgebraLimits::default(),
            )
            .unwrap();
        assert_eq!(&replayed, source_coefficient);
    }
}

#[test]
fn sunset_rows_translate_with_exact_replay_and_stable_provenance() {
    let (_, family) = equal_mass_sunset("translated-source-sunset-replay");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let offset = IntegralShift::try_new([-1, 0, 0]).unwrap();
    let translated = generator
        .translate_completed_source_rows(&completed, [offset], TranslatedSourceLimits::default())
        .unwrap();

    assert_eq!(translated.source_row_count(), 4);
    assert_eq!(translated.offsets()[0].values(), &[-1, 0, 0]);
    assert_eq!(translated.len(), 4);
    for source_ordinal in [0, 3] {
        exact_translation_replays(
            generator.context(),
            &completed.relations[source_ordinal],
            &translated.sources()[source_ordinal],
            &[-1, 0, 0],
        );
    }
    assert_eq!(
        translated.sources()[0].provenance().stable_string(),
        "translated-source-v1:0:ordinary-ibp:0:0:[-1,0,0]"
    );
    assert_eq!(
        translated.sources()[3].provenance().stable_string(),
        "translated-source-v1:3:ordinary-ibp:1:1:[-1,0,0]"
    );
}

#[test]
fn offsets_are_sorted_deduplicated_and_zero_is_an_exact_identity() {
    let (_, family) = equal_mass_sunset("translated-source-order");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let zero = IntegralShift::try_new([0, 0, 0]).unwrap();
    let lower = IntegralShift::try_new([-1, 0, 0]).unwrap();
    let translated = generator
        .translate_completed_source_rows(
            &completed,
            [zero.clone(), lower.clone(), zero, lower],
            TranslatedSourceLimits::default(),
        )
        .unwrap();

    assert_eq!(
        translated
            .offsets()
            .iter()
            .map(|offset| offset.values())
            .collect::<Vec<_>>(),
        vec![&[-1, 0, 0][..], &[0, 0, 0][..]]
    );
    assert_eq!(translated.len(), 8);
    assert_eq!(
        translated
            .sources()
            .iter()
            .map(|source| source.row_id().stable_string())
            .collect::<Vec<_>>(),
        vec![
            "ordinary-ibp:0:0",
            "ordinary-ibp:0:1",
            "ordinary-ibp:1:0",
            "ordinary-ibp:1:1",
            "ordinary-ibp:0:0",
            "ordinary-ibp:0:1",
            "ordinary-ibp:1:0",
            "ordinary-ibp:1:1",
        ]
    );
    for (source, identity) in completed.relations.iter().zip(&translated.sources()[4..]) {
        assert_eq!(identity.terms(), source.terms());
        assert_eq!(identity.nonzero_conditions(), source.nonzero_conditions());
        assert_eq!(identity.provenance().offset().values(), &[0, 0, 0]);
    }
}

#[test]
fn translated_conditions_retain_the_exact_offset_sources() {
    let family = guarded_tadpole("translated-source-guard-provenance");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    assert!(!completed.relations[0].nonzero_conditions().is_empty());
    let translated = generator
        .translate_completed_source_rows(
            &completed,
            [IntegralShift::try_new([-1]).unwrap()],
            TranslatedSourceLimits::default(),
        )
        .unwrap();
    let sources = translated.sources()[0]
        .nonzero_conditions()
        .iter()
        .flat_map(|condition| condition.sources())
        .collect::<Vec<_>>();
    assert!(sources.iter().any(|source| matches!(
        source,
        IdentityConditionSource::IndexTranslation { offset } if offset.as_ref() == [-1]
    )));
    assert!(sources.iter().any(|source| matches!(
        source,
        IdentityConditionSource::RelationTranslation {
            source_row: RowId::OrdinaryIbp {
                contraction_momentum: 0,
                differentiated_loop: 0,
            },
            target_row: RowId::OrdinaryIbp {
                contraction_momentum: 0,
                differentiated_loop: 0,
            },
            offset,
        } if offset.as_ref() == [-1]
    )));
}

#[test]
fn aggregate_condition_source_limit_has_exact_boundary_and_checked_overflow() {
    let family = guarded_tadpole("translated-source-condition-source-bound");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let offsets = vec![
        IntegralShift::try_new([0]).unwrap(),
        IntegralShift::try_new([-1]).unwrap(),
    ];
    let zero_requested =
        retained_condition_source_entry_bound(&completed, std::slice::from_ref(&offsets[0]))
            .unwrap();
    let existing = completed
        .relations
        .iter()
        .flat_map(|relation| relation.nonzero_conditions())
        .map(|condition| condition.sources().len())
        .sum::<usize>();
    assert_eq!(zero_requested, existing);

    let mut zero_exact = TranslatedSourceLimits::default().with_diagnostics();
    zero_exact.max_retained_condition_source_entries = zero_requested;
    generator
        .translate_completed_source_rows(&completed, [offsets[0].clone()], zero_exact)
        .unwrap();
    let mut zero_one_below = zero_exact;
    zero_one_below.max_retained_condition_source_entries = zero_requested - 1;
    assert_eq!(
        generator
            .translate_completed_source_rows(&completed, [offsets[0].clone()], zero_one_below,),
        Err(TranslatedSourceError::ResourceLimit {
            resource: "translated-source retained condition-source entries",
            requested: zero_requested,
            limit: zero_requested - 1,
        })
    );

    let requested = retained_condition_source_entry_bound(&completed, &offsets).unwrap();
    assert!(requested > 0);

    let mut exact = TranslatedSourceLimits::default().with_diagnostics();
    exact.max_retained_condition_source_entries = requested;
    let translated = generator
        .translate_completed_source_rows(&completed, offsets.clone(), exact)
        .unwrap();
    let retained = translated
        .sources()
        .iter()
        .flat_map(|source| source.nonzero_conditions())
        .map(|condition| condition.sources().len())
        .sum::<usize>();
    assert!(
        retained <= requested,
        "the admitted provenance bound must cover every retained source"
    );

    let mut one_below = exact;
    one_below.max_retained_condition_source_entries = requested - 1;
    assert_eq!(
        generator.translate_completed_source_rows(&completed, offsets, one_below),
        Err(TranslatedSourceError::ResourceLimit {
            resource: "translated-source retained condition-source entries",
            requested,
            limit: requested - 1,
        })
    );
    assert_eq!(
        add_condition_source_entries(usize::MAX, 1),
        Err(TranslatedSourceError::ResourceCountOverflow {
            resource: "translated-source retained condition-source entries",
        })
    );
}

#[test]
fn zero_translation_reapplies_current_relation_limits_once() {
    let family = guarded_tadpole("translated-source-zero-relation-limits");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let condition_sources = completed.relations[0].nonzero_conditions()[0]
        .sources()
        .len();
    assert!(condition_sources > 0);

    let mut condition_limited = TranslatedSourceLimits::default().with_diagnostics();
    condition_limited.relation.identity_conditions.max_sources = condition_sources - 1;
    assert!(matches!(
        generator.translate_completed_source_rows(
            &completed,
            [IntegralShift::try_new([0]).unwrap()],
            condition_limited,
        ),
        Err(TranslatedSourceError::RelationTranslation {
            offset_ordinal: 0,
            source_ordinal: 0,
            error: ParametricRelationError::IdentityCondition(
                IdentityConditionError::ResourceLimit {
                    resource: "identity condition sources",
                    requested,
                    limit,
                }
            ),
        }) if requested == condition_sources && limit == condition_sources - 1
    ));

    let mut algebra_limited = TranslatedSourceLimits::default().with_diagnostics();
    algebra_limited
        .relation
        .arithmetic
        .exact_algebra
        .max_polynomial_terms = 0;
    assert!(matches!(
        generator.translate_completed_source_rows(
            &completed,
            [IntegralShift::try_new([0]).unwrap()],
            algebra_limited,
        ),
        Err(TranslatedSourceError::RelationTranslation {
            offset_ordinal: 0,
            source_ordinal: 0,
            error: ParametricRelationError::Coefficient(
                IndexedAlgebraError::ExactAlgebra(ExactAlgebraError::ResourceLimit {
                    resource: "authenticated polynomial terms",
                    requested,
                    limit: 0,
                })
            ),
        }) if requested > 0
    ));
}

#[test]
fn nonzero_translation_authenticates_each_native_result_once_without_operand_rescans() {
    let family = guarded_tadpole("translated-source-sealed-authentication");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let translated_coefficients = completed
        .relations
        .iter()
        .map(|relation| relation.terms().len())
        .sum::<usize>();
    let before = generator.context().authentication_scan_counts();
    generator
        .translate_completed_source_rows(
            &completed,
            [IntegralShift::try_new([-1]).unwrap()],
            TranslatedSourceLimits::default(),
        )
        .unwrap();
    let after = generator.context().authentication_scan_counts();

    assert_eq!(
        after.0 - before.0,
        0,
        "sealed operands must not be rescanned"
    );
    assert_eq!(
        after.1 - before.1,
        translated_coefficients,
        "each translated native coefficient result is authenticated exactly once"
    );
}

#[test]
fn foreign_family_and_context_fail_before_translation() {
    let (_, family) = equal_mass_sunset("translated-source-owned");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let (_, foreign_family) = equal_mass_sunset("translated-source-foreign");
    let foreign_generator = ParametricIbpGenerator::try_new(&foreign_family).unwrap();
    let foreign = complete_ordinary(&foreign_generator);
    assert_eq!(
        generator.translate_completed_source_rows(
            &foreign,
            [IntegralShift::try_new([0, 0, 0]).unwrap()],
            TranslatedSourceLimits::default(),
        ),
        Err(TranslatedSourceError::CompletedSourceFamilyMismatch)
    );

    let mut wrong_context = complete_ordinary(&generator);
    wrong_context.scope.context_fingerprint = Arc::new("foreign-context".to_owned());
    assert_eq!(
        generator.translate_completed_source_rows(
            &wrong_context,
            [IntegralShift::try_new([0, 0, 0]).unwrap()],
            TranslatedSourceLimits::default(),
        ),
        Err(TranslatedSourceError::CompletedSourceContextMismatch)
    );
}

#[test]
fn construction_and_batch_bounds_fail_with_exact_typed_errors() {
    assert_eq!(
        IntegralShift::try_new([]),
        Err(TranslatedSourceError::EmptyIntegralShift)
    );
    assert_eq!(
        IntegralShift::try_new_with_component_limit([0, 0], 1),
        Err(TranslatedSourceError::ResourceLimit {
            resource: "integral-shift components",
            requested: 2,
            limit: 1,
        })
    );

    let (_, family) = equal_mass_sunset("translated-source-bounds");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    assert_eq!(
        generator.translate_completed_source_rows(
            &completed,
            [IntegralShift::try_new([0, 0]).unwrap()],
            TranslatedSourceLimits::default(),
        ),
        Err(TranslatedSourceError::WrongOffsetArity {
            offset_ordinal: 0,
            expected: 3,
            actual: 2,
        })
    );

    let mut limits = TranslatedSourceLimits::default().with_diagnostics();
    limits.max_requested_offsets = 1;
    assert_eq!(
        generator.translate_completed_source_rows(
            &completed,
            [
                IntegralShift::try_new([0, 0, 0]).unwrap(),
                IntegralShift::try_new([0, 0, 0]).unwrap(),
            ],
            limits,
        ),
        Err(TranslatedSourceError::ResourceLimit {
            resource: "requested translated-source offsets",
            requested: 2,
            limit: 1,
        })
    );

    let mut limits = TranslatedSourceLimits::default().with_diagnostics();
    limits.max_translated_sources = 3;
    assert_eq!(
        generator.translate_completed_source_rows(
            &completed,
            [IntegralShift::try_new([0, 0, 0]).unwrap()],
            limits,
        ),
        Err(TranslatedSourceError::ResourceLimit {
            resource: "translated source rows",
            requested: 4,
            limit: 3,
        })
    );

    let mut limits = TranslatedSourceLimits::default().with_diagnostics();
    limits.max_retained_index_coordinate_cells = 0;
    assert!(matches!(
        generator.translate_completed_source_rows(
            &completed,
            [IntegralShift::try_new([0, 0, 0]).unwrap()],
            limits,
        ),
        Err(TranslatedSourceError::ResourceLimit {
            resource: "translated-source retained index-coordinate cells",
            requested,
            limit: 0,
        }) if requested > 0
    ));
}

#[test]
fn index_overflow_and_symbolic_work_fail_through_typed_translation_context() {
    let (_, family) = equal_mass_sunset("translated-source-overflow");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    assert!(matches!(
        generator.translate_completed_source_rows(
            &completed,
            [IntegralShift::try_new([i64::MAX, 0, 0]).unwrap()],
            TranslatedSourceLimits::default(),
        ),
        Err(TranslatedSourceError::RelationTranslation {
            offset_ordinal: 0,
            source_ordinal: _,
            error: ParametricRelationError::IndexOverflow { position: 0 },
        })
    ));

    let mut limits = TranslatedSourceLimits::default().with_diagnostics();
    limits.relation.arithmetic.max_specialization_integer_bits = 0;
    assert!(matches!(
        generator.translate_completed_source_rows(
            &completed,
            [IntegralShift::try_new([-1, 0, 0]).unwrap()],
            limits,
        ),
        Err(TranslatedSourceError::RelationTranslation {
            offset_ordinal: 0,
            source_ordinal: _,
            error: ParametricRelationError::Coefficient(IndexedAlgebraError::ResourceLimit {
                resource: "parametric translation integer bits",
                ..
            }),
        })
    ));
}

#[test]
fn empty_complete_source_batches_are_rejected() {
    let base = CoefficientContext::new(["d"]);
    let family = IntegralFamily::new(
        "translated-source-empty",
        vec!["k".into()],
        Vec::new(),
        base.clone(),
        base.parameter("d").unwrap(),
        vec![AffineDenominator::new(base.integer(-1), vec![base.one()])],
        Vec::new(),
        vec![base.zero()],
    )
    .unwrap();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let external = generator.prepare_external_ibp_sources().unwrap();
    assert_eq!(external.len(), 0);
    let completed = external.complete(Vec::new()).unwrap();
    assert_eq!(
        generator.translate_completed_source_rows(
            &completed,
            [IntegralShift::try_new([0]).unwrap()],
            TranslatedSourceLimits::default(),
        ),
        Err(TranslatedSourceError::EmptySourceRows)
    );
}

mod selected;
