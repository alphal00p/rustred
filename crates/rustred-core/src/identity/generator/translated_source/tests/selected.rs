use crate::identity::{ParametricIbpGenerator, ParametricRelationError, TranslatedSourceRequest};

use super::{
    TranslatedSourceError, TranslatedSourceLimits, complete_ordinary, equal_mass_sunset,
    guarded_tadpole,
};
use crate::identity::IntegralShift;

use super::super::construction::{
    retained_condition_source_entry_bound_for, retained_coordinate_cell_bound_for,
};

fn request(source_ordinal: usize, offset: [i64; 3]) -> TranslatedSourceRequest {
    TranslatedSourceRequest::new(source_ordinal, IntegralShift::try_new(offset).unwrap())
}

#[test]
fn selected_signed_subset_is_canonical_deduplicated_and_matches_rectangular_rows() {
    let (_, family) = equal_mass_sunset("selected-translated-source-canonical");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let lower = [-1, 0, 1];
    let zero = [0, 0, 0];
    let upper = [1, -2, 0];

    let selected = generator
        .translate_selected_completed_source_rows(
            &completed,
            [
                request(3, upper),
                request(1, lower),
                request(0, upper),
                request(2, zero),
                request(1, lower),
            ],
            TranslatedSourceLimits::default(),
        )
        .unwrap();
    assert_eq!(selected.completed_source_row_count(), 4);
    assert_eq!(selected.len(), 4);
    assert!(!selected.is_empty());
    assert_eq!(selected.family_fingerprint(), family.fingerprint());
    assert_eq!(
        selected.context_fingerprint(),
        generator.context().fingerprint()
    );
    assert_eq!(
        selected
            .requests()
            .iter()
            .map(|request| (request.source_ordinal(), request.offset().values()))
            .collect::<Vec<_>>(),
        vec![
            (1, &lower[..]),
            (2, &zero[..]),
            (0, &upper[..]),
            (3, &upper[..]),
        ]
    );

    let rectangular = generator
        .translate_completed_source_rows(
            &completed,
            [
                IntegralShift::try_new(upper).unwrap(),
                IntegralShift::try_new(lower).unwrap(),
                IntegralShift::try_new(zero).unwrap(),
            ],
            TranslatedSourceLimits::default(),
        )
        .unwrap();
    for (request, source) in selected.requests().iter().zip(selected.sources()) {
        let offset_ordinal = rectangular
            .offsets()
            .binary_search(request.offset())
            .unwrap();
        let rectangular_ordinal =
            offset_ordinal * rectangular.source_row_count() + request.source_ordinal();
        assert_eq!(source, &rectangular.sources()[rectangular_ordinal]);
        assert_eq!(
            source.provenance().source_ordinal(),
            request.source_ordinal()
        );
        assert_eq!(source.provenance().offset(), request.offset());
    }

    let reordered = generator
        .translate_selected_completed_source_rows(
            &completed,
            [
                request(0, upper),
                request(1, lower),
                request(3, upper),
                request(2, zero),
                request(0, upper),
            ],
            TranslatedSourceLimits::default(),
        )
        .unwrap();
    assert_eq!(selected, reordered);
}

#[test]
fn selected_subset_translates_no_unrequested_rows() {
    let (_, family) = equal_mass_sunset("selected-translated-source-subset");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let source_ordinal = 3;
    let selected_term_count = completed.relations[source_ordinal].terms().len();
    let all_term_count = completed
        .relations
        .iter()
        .map(|relation| relation.terms().len())
        .sum::<usize>();
    assert!(selected_term_count < all_term_count);

    let before = generator.context().authentication_scan_counts();
    let translated = generator
        .translate_selected_completed_source_rows(
            &completed,
            [request(source_ordinal, [-1, 0, 0])],
            TranslatedSourceLimits::default(),
        )
        .unwrap();
    let after = generator.context().authentication_scan_counts();
    assert_eq!(translated.len(), 1);
    assert_eq!(translated.sources()[0].provenance().source_ordinal(), 3);
    assert_eq!(after.0 - before.0, 0);
    assert_eq!(after.1 - before.1, selected_term_count);
}

#[test]
fn selected_request_validation_and_raw_duplicate_budget_fail_typed() {
    let (_, family) = equal_mass_sunset("selected-translated-source-validation");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);

    assert_eq!(
        generator.translate_selected_completed_source_rows(
            &completed,
            [],
            TranslatedSourceLimits::default(),
        ),
        Err(TranslatedSourceError::EmptySourceRequests)
    );
    assert_eq!(
        generator.translate_selected_completed_source_rows(
            &completed,
            [TranslatedSourceRequest::new(
                0,
                IntegralShift::try_new([0, 0]).unwrap(),
            )],
            TranslatedSourceLimits::default(),
        ),
        Err(TranslatedSourceError::WrongRequestOffsetArity {
            request_ordinal: 0,
            expected: 3,
            actual: 2,
        })
    );
    assert_eq!(
        generator.translate_selected_completed_source_rows(
            &completed,
            [request(0, [0, 0, 0]), request(4, [0, 0, 0])],
            TranslatedSourceLimits::default(),
        ),
        Err(TranslatedSourceError::SourceOrdinalOutOfRange {
            request_ordinal: 1,
            source_ordinal: 4,
            source_count: 4,
        })
    );

    let mut raw_limited = TranslatedSourceLimits::default();
    raw_limited.max_requested_source_translations = 1;
    assert_eq!(
        generator.translate_selected_completed_source_rows(
            &completed,
            [request(0, [0, 0, 0]), request(0, [0, 0, 0])],
            raw_limited,
        ),
        Err(TranslatedSourceError::ResourceLimit {
            resource: "requested selected source translations",
            requested: 2,
            limit: 1,
        })
    );

    let mut canonical_limited = TranslatedSourceLimits::default();
    canonical_limited.max_translated_sources = 1;
    assert_eq!(
        generator.translate_selected_completed_source_rows(
            &completed,
            [request(0, [0, 0, 0]), request(1, [0, 0, 0])],
            canonical_limited,
        ),
        Err(TranslatedSourceError::ResourceLimit {
            resource: "translated source rows",
            requested: 2,
            limit: 1,
        })
    );

    let mut offset_limited = TranslatedSourceLimits::default();
    offset_limited.max_requested_offsets = 1;
    assert_eq!(
        generator.translate_selected_completed_source_rows(
            &completed,
            [request(0, [-1, 0, 0]), request(1, [0, 0, 0])],
            offset_limited,
        ),
        Err(TranslatedSourceError::ResourceLimit {
            resource: "canonical selected translation offsets",
            requested: 2,
            limit: 1,
        })
    );
}

#[test]
fn selected_aggregate_caps_have_exact_and_one_below_boundaries_before_symbolic_work() {
    let family = guarded_tadpole("selected-translated-source-bounds");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let requests = [
        TranslatedSourceRequest::new(0, IntegralShift::try_new([-1]).unwrap()),
        TranslatedSourceRequest::new(0, IntegralShift::try_new([0]).unwrap()),
    ];
    let translated_terms = requests
        .iter()
        .map(|request| completed.relations[request.source_ordinal()].terms().len())
        .sum::<usize>();
    let translated_conditions = requests
        .iter()
        .map(|request| {
            completed.relations[request.source_ordinal()]
                .nonzero_conditions()
                .len()
        })
        .sum::<usize>();
    let retained_condition_sources =
        retained_condition_source_entry_bound_for(requests.iter().map(|request| {
            (
                &completed.relations[request.source_ordinal()],
                request.offset(),
            )
        }))
        .unwrap();
    let retained_coordinate_cells = retained_coordinate_cell_bound_for(
        1,
        requests.len(),
        requests.iter().map(|request| {
            (
                &completed.relations[request.source_ordinal()],
                request.offset(),
            )
        }),
    )
    .unwrap();
    assert!(translated_terms > 0);
    assert!(translated_conditions > 0);
    assert!(retained_condition_sources > 0);
    assert!(retained_coordinate_cells > 0);

    let mut exact = TranslatedSourceLimits::default();
    exact.max_requested_source_translations = requests.len();
    exact.max_requested_offsets = requests.len();
    exact.max_translated_sources = requests.len();
    exact.max_translated_term_entries = translated_terms;
    exact.max_translated_condition_entries = translated_conditions;
    exact.max_retained_condition_source_entries = retained_condition_sources;
    exact.max_retained_index_coordinate_cells = retained_coordinate_cells;
    let selected = generator
        .translate_selected_completed_source_rows(&completed, requests.clone(), exact)
        .unwrap();
    let rectangular = generator
        .translate_completed_source_rows(
            &completed,
            [
                IntegralShift::try_new([-1]).unwrap(),
                IntegralShift::try_new([0]).unwrap(),
            ],
            exact,
        )
        .unwrap();
    assert_eq!(selected.sources(), rectangular.sources());

    for (resource, requested, one_below) in [
        ("translated source term entries", translated_terms, {
            let mut limits = exact;
            limits.max_translated_term_entries = translated_terms - 1;
            limits
        }),
        (
            "translated source condition entries",
            translated_conditions,
            {
                let mut limits = exact;
                limits.max_translated_condition_entries = translated_conditions - 1;
                limits
            },
        ),
        (
            "translated-source retained condition-source entries",
            retained_condition_sources,
            {
                let mut limits = exact;
                limits.max_retained_condition_source_entries = retained_condition_sources - 1;
                limits
            },
        ),
        (
            "translated-source retained index-coordinate cells",
            retained_coordinate_cells,
            {
                let mut limits = exact;
                limits.max_retained_index_coordinate_cells = retained_coordinate_cells - 1;
                limits
            },
        ),
    ] {
        let before = generator.context().authentication_scan_counts();
        let result = generator.translate_selected_completed_source_rows(
            &completed,
            requests.clone(),
            one_below,
        );
        let after = generator.context().authentication_scan_counts();
        assert_eq!(before, after, "{resource} must fail before Symbolica work");
        assert_eq!(
            result,
            Err(TranslatedSourceError::ResourceLimit {
                resource,
                requested,
                limit: requested - 1,
            })
        );
    }
}

#[test]
fn selected_symbolic_failure_names_the_canonical_request() {
    let (_, family) = equal_mass_sunset("selected-translated-source-symbolic-error");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let result = generator.translate_selected_completed_source_rows(
        &completed,
        [request(0, [i64::MAX, 0, 0])],
        TranslatedSourceLimits::default(),
    );
    assert!(
        matches!(
            result,
            Err(TranslatedSourceError::RequestTranslation {
                canonical_request_ordinal: 0,
                source_ordinal: 0,
                error: ParametricRelationError::IndexOverflow { position: 0 },
            })
        ),
        "unexpected result: {result:?}"
    );
}
