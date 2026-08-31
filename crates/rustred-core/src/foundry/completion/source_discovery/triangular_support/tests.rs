use std::collections::BTreeSet;

use crate::algebra::CoefficientContext;
use crate::family::{AffineDenominator, IntegralFamily};
use crate::foundry::completion::frame::{
    OneSidedChartFrame, PhysicalFrameError, PhysicalFrameLimits, SelectedSourceFrame,
};
use crate::identity::{CompletedIbpSourceRows, ParametricIbpGenerator, TranslatedSourceError};
use crate::sector::Mask;

use super::super::CampaignLimits;
use super::{
    ACCUMULATED_REQUESTS, OFFSET_COORDINATES, OFFSETS, PHYSICAL_CSR_ROW_OFFSETS,
    PHYSICAL_SOURCE_INSTANCES, SELECTED_TRANSLATION_OFFSETS, SELECTED_TRANSLATION_REQUESTS,
    SELECTED_TRANSLATION_SOURCES, TriangularSupportError, try_build_triangular_support_frame,
};

fn sunset_family_named(name: &str) -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d"]);
    let zero = coefficients.zero();
    let one = coefficients.one();
    let minus_one = coefficients.integer(-1);
    IntegralFamily::new(
        name,
        vec!["k1".into(), "k2".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(
                minus_one.clone(),
                vec![one.clone(), zero.clone(), zero.clone()],
            ),
            AffineDenominator::new(
                minus_one.clone(),
                vec![zero.clone(), zero.clone(), one.clone()],
            ),
            AffineDenominator::new(minus_one, vec![one.clone(), coefficients.integer(2), one]),
        ],
        Vec::new(),
        vec![zero.clone(), zero.clone(), zero],
    )
    .unwrap()
}

fn sunset_family() -> IntegralFamily {
    sunset_family_named("triangular-support-sunset")
}

fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
}

fn request_set(frame: &SelectedSourceFrame) -> BTreeSet<(usize, Vec<i64>)> {
    frame
        .plan()
        .source_instances()
        .iter()
        .map(|source| {
            (
                source.provenance().source_ordinal(),
                source.provenance().offset().values().to_vec(),
            )
        })
        .collect()
}

#[test]
fn per_source_subset_is_bruteforce_complete_unique_and_provenance_exact() {
    let family = sunset_family();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    assert_eq!(completed.source_row_count(), 4);
    let ceilings = [0, 1, 2, 3];
    let sector = Mask::try_new([true, false, true]).unwrap();
    let frame = try_build_triangular_support_frame(
        &generator,
        &completed,
        sector,
        &[2, 1],
        &ceilings,
        CampaignLimits::default(),
    )
    .unwrap();

    let mut expected = BTreeSet::new();
    for (source, &ceiling) in ceilings.iter().enumerate() {
        for axis_two in 0..=ceiling {
            for axis_one in 0..=ceiling - axis_two {
                expected.insert((source, vec![0, -(axis_one as i64), axis_two as i64]));
            }
        }
    }
    assert_eq!(expected.len(), 1 + 3 + 6 + 10);
    assert_eq!(frame.plan().row_count(), expected.len());
    assert_eq!(request_set(&frame), expected);
    assert_eq!(frame.completed_source_row_count(), 4);

    for row in 0..frame.plan().row_count() {
        let instance = &frame.plan().source_instances()[row];
        let translated = frame.plan().source_for_row(row).unwrap();
        assert_eq!(translated.provenance(), instance.provenance());
        assert_eq!(
            translated.provenance().source_row(),
            completed
                .source_row_id(translated.provenance().source_ordinal())
                .unwrap()
        );
    }
}

#[test]
fn empty_axis_support_emits_exactly_one_zero_request_per_source() {
    let family = sunset_family();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let frame = try_build_triangular_support_frame(
        &generator,
        &completed,
        Mask::try_new([true, false, true]).unwrap(),
        &[],
        &[3, 2, 1, 0],
        CampaignLimits::default(),
    )
    .unwrap();

    let expected = (0..completed.source_row_count())
        .map(|source| (source, vec![0, 0, 0]))
        .collect();
    assert_eq!(request_set(&frame), expected);
    assert_eq!(frame.plan().row_count(), completed.source_row_count());
}

#[test]
fn one_axis_support_is_complete_and_uses_the_sector_sign() {
    let family = sunset_family();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let sector = Mask::try_new([true, false, true]).unwrap();
    let ceilings = [2, 0, 0, 0];
    let build = |axis| {
        try_build_triangular_support_frame(
            &generator,
            &completed,
            sector.clone(),
            &[axis],
            &ceilings,
            CampaignLimits::default(),
        )
        .unwrap()
    };

    let active = request_set(&build(2));
    let inactive = request_set(&build(1));
    let mut expected_active = BTreeSet::new();
    let mut expected_inactive = BTreeSet::new();
    for degree in 0..=2 {
        expected_active.insert((0, vec![0, 0, degree]));
        expected_inactive.insert((0, vec![0, -degree, 0]));
    }
    for source in 1..completed.source_row_count() {
        expected_active.insert((source, vec![0, 0, 0]));
        expected_inactive.insert((source, vec![0, 0, 0]));
    }
    assert_eq!(active, expected_active);
    assert_eq!(inactive, expected_inactive);
}

#[test]
fn full_uniform_support_matches_the_independent_chart_bruteforce() {
    let family = sunset_family();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let sector = Mask::try_new([true, false, true]).unwrap();
    let expected = OneSidedChartFrame::try_new(
        &generator,
        &completed,
        sector.clone(),
        2,
        PhysicalFrameLimits::default(),
    )
    .unwrap();
    let actual = try_build_triangular_support_frame(
        &generator,
        &completed,
        sector,
        &[0, 1, 2],
        &[2; 4],
        CampaignLimits::default(),
    )
    .unwrap();

    assert_eq!(actual.plan(), expected.plan());
    assert_eq!(actual.plan().row_count(), 4 * 10);
}

#[test]
fn axis_enumeration_order_cannot_change_the_canonical_frame() {
    let family = sunset_family();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let build = |axes: &[usize]| {
        try_build_triangular_support_frame(
            &generator,
            &completed,
            Mask::try_new([true, false, true]).unwrap(),
            axes,
            &[1, 2, 0, 3],
            CampaignLimits::default(),
        )
        .unwrap()
    };

    let first = build(&[2, 1]);
    let repeated = build(&[2, 1]);
    let permuted = build(&[1, 2]);
    assert_eq!(first, repeated);
    assert_eq!(first, permuted);
    assert_eq!(
        first.plan().row_count(),
        request_set(&first).len(),
        "canonical selected frame must retain no duplicate request"
    );
}

#[test]
fn malformed_axes_and_source_ceiling_tables_fail_closed() {
    let family = sunset_family();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let sector = Mask::try_new([true, false, true]).unwrap();
    let build = |axes: &[usize], ceilings: &[usize]| {
        try_build_triangular_support_frame(
            &generator,
            &completed,
            sector.clone(),
            axes,
            ceilings,
            CampaignLimits::default(),
        )
    };

    assert_eq!(
        build(&[1, 0, 1], &[1; 4]).unwrap_err(),
        TriangularSupportError::DuplicateAxis {
            first_position: 0,
            duplicate_position: 2,
            axis: 1,
        }
    );
    assert_eq!(
        build(&[3], &[1; 4]).unwrap_err(),
        TriangularSupportError::AxisOutOfRange {
            position: 0,
            axis: 3,
            arity: 3,
        }
    );
    assert_eq!(
        build(&[1], &[1; 3]).unwrap_err(),
        TriangularSupportError::WrongSourceCeilingCount {
            expected: 4,
            actual: 3,
        }
    );
}

#[test]
fn sector_arity_and_completed_source_scope_are_checked_before_enumeration() {
    let family = sunset_family();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    assert_eq!(
        try_build_triangular_support_frame(
            &generator,
            &completed,
            Mask::try_new([true, false]).unwrap(),
            &[0],
            &[0; 4],
            CampaignLimits::default(),
        )
        .unwrap_err(),
        TriangularSupportError::WrongSectorArity {
            expected: 3,
            actual: 2,
        }
    );

    let foreign_family = sunset_family_named("triangular-support-foreign-sunset");
    let foreign_generator = ParametricIbpGenerator::try_new(&foreign_family).unwrap();
    let foreign_completed = complete_ordinary(&foreign_generator);
    assert_eq!(
        try_build_triangular_support_frame(
            &generator,
            &foreign_completed,
            Mask::try_new([true, false, true]).unwrap(),
            &[0],
            &[0; 4],
            CampaignLimits::default(),
        )
        .unwrap_err(),
        TriangularSupportError::SourceTranslation(
            TranslatedSourceError::CompletedSourceFamilyMismatch,
        )
    );
}

#[test]
fn request_and_offset_resources_admit_exact_boundaries() {
    let family = sunset_family();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let sector = Mask::try_new([true, false, true]).unwrap();
    let ceilings = [0, 1, 2, 3];
    let mut exact = CampaignLimits::default();
    exact.max_request_arity = 3;
    exact.max_submitted_requests = 20;
    exact.max_canonical_candidate_requests = 20;
    exact.max_accumulated_requests = 20;
    exact.max_request_coordinate_cells = 60;
    exact.physical_frame.max_arity = 3;
    exact.physical_frame.max_degree = 3;
    exact.physical_frame.max_offsets = 10;
    exact.physical_frame.max_offset_coordinate_cells = 30;
    try_build_triangular_support_frame(
        &generator,
        &completed,
        sector.clone(),
        &[2, 1],
        &ceilings,
        exact,
    )
    .unwrap();

    let mut requests_one_below = exact;
    requests_one_below.max_accumulated_requests = 19;
    assert_eq!(
        try_build_triangular_support_frame(
            &generator,
            &completed,
            sector.clone(),
            &[2, 1],
            &ceilings,
            requests_one_below,
        )
        .unwrap_err(),
        TriangularSupportError::ResourceLimit {
            resource: ACCUMULATED_REQUESTS,
            requested: 20,
            limit: 19,
        }
    );

    let mut offsets_one_below = exact;
    offsets_one_below.physical_frame.max_offset_coordinate_cells = 29;
    assert_eq!(
        try_build_triangular_support_frame(
            &generator,
            &completed,
            sector,
            &[2, 1],
            &ceilings,
            offsets_one_below,
        )
        .unwrap_err(),
        TriangularSupportError::ResourceLimit {
            resource: OFFSET_COORDINATES,
            requested: 30,
            limit: 29,
        }
    );
}

#[test]
fn nested_selected_translation_and_physical_caps_preflight_one_below_exact() {
    let family = sunset_family();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let sector = Mask::try_new([true, false, true]).unwrap();
    let ceilings = [0, 1, 2, 3];
    let mut exact = CampaignLimits::default();
    exact.translated_sources.max_requested_source_translations = 20;
    exact.translated_sources.max_translated_sources = 20;
    exact.translated_sources.max_requested_offsets = 10;
    exact.physical_frame.max_source_instances = 20;
    exact.physical_frame.max_csr_row_offsets = 21;
    let build = |limits| {
        try_build_triangular_support_frame(
            &generator,
            &completed,
            sector.clone(),
            &[2, 1],
            &ceilings,
            limits,
        )
    };
    build(exact).unwrap();

    let mut request_cap = exact;
    request_cap
        .translated_sources
        .max_requested_source_translations = 19;
    assert_eq!(
        build(request_cap).unwrap_err(),
        TriangularSupportError::SourceTranslation(TranslatedSourceError::ResourceLimit {
            resource: SELECTED_TRANSLATION_REQUESTS,
            requested: 20,
            limit: 19,
        })
    );

    let mut source_cap = exact;
    source_cap.translated_sources.max_translated_sources = 19;
    assert_eq!(
        build(source_cap).unwrap_err(),
        TriangularSupportError::SourceTranslation(TranslatedSourceError::ResourceLimit {
            resource: SELECTED_TRANSLATION_SOURCES,
            requested: 20,
            limit: 19,
        })
    );

    let mut offset_cap = exact;
    offset_cap.translated_sources.max_requested_offsets = 9;
    assert_eq!(
        build(offset_cap).unwrap_err(),
        TriangularSupportError::SourceTranslation(TranslatedSourceError::ResourceLimit {
            resource: SELECTED_TRANSLATION_OFFSETS,
            requested: 10,
            limit: 9,
        })
    );

    let mut instance_cap = exact;
    instance_cap.physical_frame.max_source_instances = 19;
    assert_eq!(
        build(instance_cap).unwrap_err(),
        TriangularSupportError::PhysicalFrame(PhysicalFrameError::ResourceLimit {
            resource: PHYSICAL_SOURCE_INSTANCES,
            requested: 20,
            limit: 19,
        })
    );

    let mut csr_cap = exact;
    csr_cap.physical_frame.max_csr_row_offsets = 20;
    assert_eq!(
        build(csr_cap).unwrap_err(),
        TriangularSupportError::PhysicalFrame(PhysicalFrameError::ResourceLimit {
            resource: PHYSICAL_CSR_ROW_OFFSETS,
            requested: 21,
            limit: 20,
        })
    );
}

#[test]
fn combinatorial_request_overflow_is_rejected_before_allocation() {
    let family = sunset_family();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let huge = usize::try_from(i64::MAX).unwrap();
    let mut limits = CampaignLimits::default();
    limits.max_submitted_requests = usize::MAX;
    limits.max_canonical_candidate_requests = usize::MAX;
    limits.max_accumulated_requests = usize::MAX;
    limits.max_request_coordinate_cells = usize::MAX;
    limits.physical_frame.max_degree = huge;
    limits.physical_frame.max_offsets = usize::MAX;
    limits.physical_frame.max_offset_coordinate_cells = usize::MAX;

    assert_eq!(
        try_build_triangular_support_frame(
            &generator,
            &completed,
            Mask::try_new([true, false, true]).unwrap(),
            &[0, 1],
            &[huge, 0, 0, 0],
            limits,
        )
        .unwrap_err(),
        TriangularSupportError::ResourceCountOverflow { resource: OFFSETS }
    );
}
