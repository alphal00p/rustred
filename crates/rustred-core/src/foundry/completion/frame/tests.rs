use std::cmp::Ordering;
use std::collections::BTreeSet;

use crate::algebra::CoefficientContext;
use crate::family::{AffineDenominator, IntegralFamily};
use crate::foundry::artifact::canonical_three_loop_family;
use crate::identity::{
    CompletedIbpSourceRows, IntegralShift, ParametricIbpGenerator, TranslatedSourceLimits,
    TranslatedSourceRequest,
};
use crate::sector::Mask;

use super::assemble::checked_u32_for_test;
use super::{OneSidedChartFrame, PhysicalFrameError, PhysicalFrameLimits, SelectedSourceFrame};

fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
}

fn one_loop_tadpole(name: &str) -> IntegralFamily {
    let context = CoefficientContext::new(["d"]);
    IntegralFamily::new(
        name,
        vec!["k".to_owned()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        vec![AffineDenominator::new(
            context.integer(-1),
            vec![context.one()],
        )],
        Vec::new(),
        vec![context.zero()],
    )
    .unwrap()
}

fn one_loop_family_with_one_external(name: &str) -> IntegralFamily {
    let context = CoefficientContext::new(["d", "s"]);
    IntegralFamily::new(
        name,
        vec!["k".to_owned()],
        vec!["p".to_owned()],
        context.clone(),
        context.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(context.integer(-1), vec![context.one(), context.zero()]),
            AffineDenominator::new(context.zero(), vec![context.zero(), context.one()]),
        ],
        vec![vec![context.parameter("s").unwrap()]],
        vec![context.zero(), context.zero()],
    )
    .unwrap()
}

fn guarded_tadpole(name: &str) -> IntegralFamily {
    let context = CoefficientContext::new(["d", "x"]);
    let reciprocal = context
        .try_div(
            &context.one(),
            &context.parameter("x").unwrap(),
            Default::default(),
        )
        .unwrap();
    IntegralFamily::new(
        name,
        vec!["k".to_owned()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        vec![AffineDenominator::new(
            context.integer(-1),
            vec![context.one()],
        )],
        Vec::new(),
        vec![reciprocal],
    )
    .unwrap()
}

#[test]
fn k6_frames_regenerate_every_audited_degree_one_through_three_count() {
    let family = canonical_three_loop_family().unwrap();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let cases = [
        (
            [true, true, true, true, true, true],
            [
                (1, 63, 136, 630),
                (2, 252, 396, 2_520),
                (3, 756, 936, 7_560),
            ],
        ),
        (
            [false, true, true, true, true, true],
            [
                (1, 63, 153, 630),
                (2, 252, 464, 2_520),
                (3, 756, 1_115, 7_560),
            ],
        ),
        (
            [false, true, true, true, true, false],
            [
                (1, 63, 157, 630),
                (2, 252, 488, 2_520),
                (3, 756, 1_191, 7_560),
            ],
        ),
        (
            [false, false, true, true, true, true],
            [
                (1, 63, 161, 630),
                (2, 252, 500, 2_520),
                (3, 756, 1_215, 7_560),
            ],
        ),
    ];

    for (active, degrees) in cases {
        let sector = Mask::try_new(active).unwrap();
        for (degree, expected_rows, expected_columns, expected_entries) in degrees {
            let frame = OneSidedChartFrame::try_new(
                &generator,
                &completed,
                sector.clone(),
                degree,
                PhysicalFrameLimits::default(),
            )
            .unwrap();
            let plan = frame.plan();
            let expected_offsets = match degree {
                1 => 7,
                2 => 28,
                3 => 84,
                _ => unreachable!(),
            };
            assert_eq!(plan.sector(), &sector);
            assert_eq!(frame.degree(), degree);
            assert_eq!(frame.offsets().len(), expected_offsets);
            assert!(
                frame.offsets().windows(2).all(|pair| {
                    chart_frame_order(&sector, &pair[0], &pair[1]) == Ordering::Less
                })
            );
            assert_eq!(plan.row_count(), expected_rows);
            assert_eq!(plan.columns().len(), expected_columns);
            assert_eq!(plan.entry_count(), expected_entries);
            assert_eq!(plan.row_offsets().len(), expected_rows + 1);
            assert_eq!(
                usize::try_from(*plan.row_offsets().last().unwrap()).unwrap(),
                expected_entries
            );
            assert_eq!(plan.source_instances().len(), expected_rows);
            assert!(plan.columns().windows(2).all(|pair| pair[0] < pair[1]));
            for (offset_ordinal, offset) in frame.offsets().iter().enumerate() {
                for source_ordinal in 0..9 {
                    let row = offset_ordinal * 9 + source_ordinal;
                    let instance = &plan.source_instances()[row];
                    assert_eq!(instance.provenance().offset(), offset);
                    assert_eq!(instance.provenance().source_ordinal(), source_ordinal);
                }
            }
            for row in 0..plan.row_count() {
                let source = plan.source_for_row(row).unwrap();
                let indices = plan.column_indices_for_row(row).unwrap();
                assert_eq!(indices.len(), source.terms().len());
                assert!(indices.windows(2).all(|pair| pair[0] < pair[1]));
                assert!(
                    source
                        .terms()
                        .values()
                        .all(|coefficient| !coefficient.is_zero())
                );
                for ((shift, _), &column) in source.terms().iter().zip(indices) {
                    assert_eq!(
                        plan.columns()[usize::try_from(column).unwrap()].values(),
                        shift.values()
                    );
                }
                assert_eq!(
                    plan.source_instances()[row].provenance(),
                    source.provenance()
                );
            }
        }
    }
}

#[test]
fn s4a_rows_are_degree_chart_lex_source_chronology_and_byte_deterministic() {
    let family = canonical_three_loop_family().unwrap();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let sector = Mask::try_new([false, true, true, true, true, false]).unwrap();
    let build = || {
        OneSidedChartFrame::try_new(
            &generator,
            &completed,
            sector.clone(),
            1,
            PhysicalFrameLimits::default(),
        )
        .unwrap()
    };
    let first = build();
    let second = build();

    let expected_offsets = [
        [0, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, -1],
        [0, 0, 0, 0, 1, 0],
        [0, 0, 0, 1, 0, 0],
        [0, 0, 1, 0, 0, 0],
        [0, 1, 0, 0, 0, 0],
        [-1, 0, 0, 0, 0, 0],
    ];
    assert_eq!(
        first
            .offsets()
            .iter()
            .map(|offset| offset.values())
            .collect::<Vec<_>>(),
        expected_offsets
            .iter()
            .map(|offset| offset.as_slice())
            .collect::<Vec<_>>()
    );
    let first_plan = first.plan();
    for (offset_ordinal, expected_offset) in expected_offsets.iter().enumerate() {
        for source_ordinal in 0..9 {
            let row = offset_ordinal * 9 + source_ordinal;
            let instance = &first_plan.source_instances()[row];
            assert_eq!(instance.provenance().source_ordinal(), source_ordinal);
            assert_eq!(instance.provenance().offset().values(), expected_offset);
        }
    }

    assert_eq!(
        stable_structure_bytes(&first),
        stable_structure_bytes(&second)
    );
    assert_eq!(first, second);
}

#[test]
fn frame_owned_limits_admit_exact_boundaries_and_reject_one_below() {
    let family = one_loop_tadpole("physical-frame-limit-tadpole");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let sector = Mask::try_new([true]).unwrap();
    let baseline = OneSidedChartFrame::try_new(
        &generator,
        &completed,
        sector.clone(),
        1,
        PhysicalFrameLimits::default(),
    )
    .unwrap();

    let mut exact = PhysicalFrameLimits::default();
    exact.max_arity = 1;
    exact.max_degree = 1;
    exact.max_offsets = baseline.offsets().len();
    exact.max_offset_coordinate_cells = baseline.offsets().len();
    exact.max_source_instances = baseline.plan().row_count();
    exact.max_physical_columns = baseline.plan().columns().len();
    exact.max_physical_column_coordinate_cells = baseline.plan().columns().len();
    exact.max_physical_entries = baseline.plan().entry_count();
    exact.max_csr_row_offsets = baseline.plan().row_offsets().len();
    OneSidedChartFrame::try_new(&generator, &completed, sector.clone(), 1, exact).unwrap();

    let checks = [
        ("physical-frame arity", LimitField::Arity, 1),
        ("physical-frame degree", LimitField::Degree, 1),
        (
            "physical-frame chart offsets",
            LimitField::Offsets,
            baseline.offsets().len(),
        ),
        (
            "physical-frame offset coordinate cells",
            LimitField::OffsetCoordinates,
            baseline.offsets().len(),
        ),
        (
            "physical-frame source instances",
            LimitField::Sources,
            baseline.plan().row_count(),
        ),
        (
            "physical-frame physical columns",
            LimitField::Columns,
            baseline.plan().columns().len(),
        ),
        (
            "physical-frame physical-column coordinate cells",
            LimitField::ColumnCoordinates,
            baseline.plan().columns().len(),
        ),
        (
            "physical-frame physical entries",
            LimitField::Entries,
            baseline.plan().entry_count(),
        ),
        (
            "physical-frame CSR row offsets",
            LimitField::RowOffsets,
            baseline.plan().row_offsets().len(),
        ),
    ];
    for (resource, field, requested) in checks {
        assert!(requested > 0);
        let mut one_below = exact;
        field.set(&mut one_below, requested - 1);
        assert_eq!(
            OneSidedChartFrame::try_new(&generator, &completed, sector.clone(), 1, one_below,),
            Err(PhysicalFrameError::ResourceLimit {
                resource,
                requested,
                limit: requested - 1,
            })
        );
    }
}

#[test]
fn frame_rejects_wrong_sector_arity_and_checked_u32_overflow() {
    let family = one_loop_tadpole("physical-frame-arity-tadpole");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    assert_eq!(
        OneSidedChartFrame::try_new(
            &generator,
            &completed,
            Mask::try_new([true, false]).unwrap(),
            1,
            PhysicalFrameLimits::default(),
        ),
        Err(PhysicalFrameError::WrongSectorArity {
            expected: 1,
            actual: 2,
        })
    );

    if let Some(overflow) = usize::try_from(u32::MAX).unwrap().checked_add(1) {
        assert_eq!(
            checked_u32_for_test("physical-frame test u32", overflow),
            Err(PhysicalFrameError::U32NotRepresentable {
                resource: "physical-frame test u32",
                value: overflow,
            })
        );
    }
}

#[test]
fn frame_rejects_an_external_only_source_barrier() {
    let family = one_loop_family_with_one_external("physical-frame-source-layout");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let prepared = generator.prepare_external_ibp_sources().unwrap();
    assert_eq!(prepared.len(), 1);
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    let completed = prepared.complete(rows).unwrap();

    assert_eq!(
        OneSidedChartFrame::try_new(
            &generator,
            &completed,
            Mask::try_new([true, true]).unwrap(),
            1,
            PhysicalFrameLimits::default(),
        ),
        Err(PhysicalFrameError::WrongSourceLayout {
            actual: "external-contraction IBP source",
        })
    );
}

#[test]
fn selected_full_chart_is_exactly_the_rectangular_physical_plan() {
    let family = one_loop_family_with_one_external("physical-frame-selected-cross-path");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let sector = Mask::try_new([true, false]).unwrap();
    let chart = OneSidedChartFrame::try_new(
        &generator,
        &completed,
        sector.clone(),
        1,
        PhysicalFrameLimits::default(),
    )
    .unwrap();
    let source_count = chart.plan().row_count() / chart.offsets().len();
    assert_eq!(source_count, 2);

    let requests = chart
        .offsets()
        .iter()
        .rev()
        .flat_map(|offset| {
            (0..source_count)
                .rev()
                .map(move |source| TranslatedSourceRequest::new(source, offset.clone()))
        })
        .collect::<Vec<_>>();
    let selected = generator
        .translate_selected_completed_source_rows(
            &completed,
            requests,
            TranslatedSourceLimits::default(),
        )
        .unwrap();
    let selected =
        SelectedSourceFrame::try_new(selected, sector, PhysicalFrameLimits::default()).unwrap();

    assert_eq!(selected.completed_source_row_count(), source_count);
    let selected_plan = selected.into_plan();
    assert_eq!(&selected_plan, chart.plan());
}

#[test]
fn selected_sparse_plan_keeps_signed_identity_and_only_exact_source_columns() {
    let family = one_loop_family_with_one_external("physical-frame-selected-sparse");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let requests = [
        TranslatedSourceRequest::new(0, IntegralShift::try_new([-1, 0]).unwrap()),
        TranslatedSourceRequest::new(0, IntegralShift::try_new([1, 0]).unwrap()),
    ];
    let translated = generator
        .translate_selected_completed_source_rows(
            &completed,
            requests,
            TranslatedSourceLimits::default(),
        )
        .unwrap();
    assert_eq!(translated.completed_source_row_count(), 2);
    assert_eq!(translated.len(), 2);
    let exact_columns = translated
        .sources()
        .iter()
        .flat_map(|source| source.terms().keys())
        .map(|shift| shift.values().to_vec())
        .collect::<BTreeSet<_>>();

    let selected = SelectedSourceFrame::try_new(
        translated,
        Mask::try_new([true, false]).unwrap(),
        PhysicalFrameLimits::default(),
    )
    .unwrap();
    let plan = selected.plan();
    assert_eq!(plan.row_count(), 2);
    // Two offsets over a two-row completed source span would have four rows;
    // the selected builder owns only the two explicitly translated pairs.
    assert_ne!(plan.row_count(), 2 * selected.completed_source_row_count());
    assert_eq!(
        plan.columns()
            .iter()
            .map(|shift| shift.values().to_vec())
            .collect::<BTreeSet<_>>(),
        exact_columns
    );
    assert!(
        plan.columns()
            .iter()
            .all(|shift| shift.values() != [i64::MAX, i64::MIN])
    );

    let negative = &plan.source_instances()[0];
    let positive = &plan.source_instances()[1];
    assert_eq!(negative.provenance().offset().values(), [-1, 0]);
    assert_eq!(positive.provenance().offset().values(), [1, 0]);
    assert_ne!(negative, positive);
    assert_eq!(
        negative.stable_string(),
        format!(
            "physical-frame-source-v2:{}",
            negative.provenance().stable_string()
        )
    );
    assert_eq!(
        positive.stable_string(),
        format!(
            "physical-frame-source-v2:{}",
            positive.provenance().stable_string()
        )
    );
}

#[test]
fn selected_plan_preserves_exact_terms_guards_and_provenance_chronology() {
    let family = guarded_tadpole("physical-frame-selected-guards");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let requests = [
        TranslatedSourceRequest::new(0, IntegralShift::try_new([1]).unwrap()),
        TranslatedSourceRequest::new(0, IntegralShift::try_new([-1]).unwrap()),
    ];
    let expected = generator
        .translate_selected_completed_source_rows(
            &completed,
            requests.clone(),
            TranslatedSourceLimits::default(),
        )
        .unwrap();
    assert!(
        expected
            .sources()
            .iter()
            .all(|source| !source.nonzero_conditions().is_empty())
    );
    let actual = generator
        .translate_selected_completed_source_rows(
            &completed,
            requests,
            TranslatedSourceLimits::default(),
        )
        .unwrap();
    let selected = SelectedSourceFrame::try_new(
        actual,
        Mask::try_new([true]).unwrap(),
        PhysicalFrameLimits::default(),
    )
    .unwrap();

    for row in 0..selected.plan().row_count() {
        let actual = selected.plan().source_for_row(row).unwrap();
        let expected = expected
            .sources()
            .iter()
            .find(|source| source.provenance() == actual.provenance())
            .unwrap();
        assert_eq!(actual.terms(), expected.terms());
        assert_eq!(actual.nonzero_conditions(), expected.nonzero_conditions());
    }
}

#[test]
fn selected_physical_caps_admit_exact_boundaries_and_reject_one_below() {
    let family = guarded_tadpole("physical-frame-selected-limits");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let requests = || {
        [
            TranslatedSourceRequest::new(0, IntegralShift::try_new([-1]).unwrap()),
            TranslatedSourceRequest::new(0, IntegralShift::try_new([1]).unwrap()),
        ]
    };
    let translate = || {
        generator
            .translate_selected_completed_source_rows(
                &completed,
                requests(),
                TranslatedSourceLimits::default(),
            )
            .unwrap()
    };
    let sector = Mask::try_new([true]).unwrap();
    let baseline =
        SelectedSourceFrame::try_new(translate(), sector.clone(), PhysicalFrameLimits::default())
            .unwrap();

    let mut exact = PhysicalFrameLimits::default();
    exact.max_arity = 1;
    // Chart-only caps do not reinterpret an already selected source owner.
    exact.max_degree = 0;
    exact.max_offsets = 0;
    exact.max_offset_coordinate_cells = 0;
    exact.max_source_instances = baseline.plan().row_count();
    exact.max_physical_columns = baseline.plan().columns().len();
    exact.max_physical_column_coordinate_cells = baseline.plan().columns().len();
    exact.max_physical_entries = baseline.plan().entry_count();
    exact.max_csr_row_offsets = baseline.plan().row_offsets().len();
    SelectedSourceFrame::try_new(translate(), sector.clone(), exact).unwrap();

    for (resource, field, requested) in [
        ("physical-frame arity", LimitField::Arity, 1),
        (
            "physical-frame source instances",
            LimitField::Sources,
            baseline.plan().row_count(),
        ),
        (
            "physical-frame physical columns",
            LimitField::Columns,
            baseline.plan().columns().len(),
        ),
        (
            "physical-frame physical-column coordinate cells",
            LimitField::ColumnCoordinates,
            baseline.plan().columns().len(),
        ),
        (
            "physical-frame physical entries",
            LimitField::Entries,
            baseline.plan().entry_count(),
        ),
        (
            "physical-frame CSR row offsets",
            LimitField::RowOffsets,
            baseline.plan().row_offsets().len(),
        ),
    ] {
        assert!(requested > 0);
        let mut one_below = exact;
        field.set(&mut one_below, requested - 1);
        assert_eq!(
            SelectedSourceFrame::try_new(translate(), sector.clone(), one_below),
            Err(PhysicalFrameError::ResourceLimit {
                resource,
                requested,
                limit: requested - 1,
            })
        );
    }
}

#[derive(Clone, Copy)]
enum LimitField {
    Arity,
    Degree,
    Offsets,
    OffsetCoordinates,
    Sources,
    Columns,
    ColumnCoordinates,
    Entries,
    RowOffsets,
}

impl LimitField {
    fn set(self, limits: &mut PhysicalFrameLimits, value: usize) {
        match self {
            Self::Arity => limits.max_arity = value,
            Self::Degree => limits.max_degree = value,
            Self::Offsets => limits.max_offsets = value,
            Self::OffsetCoordinates => limits.max_offset_coordinate_cells = value,
            Self::Sources => limits.max_source_instances = value,
            Self::Columns => limits.max_physical_columns = value,
            Self::ColumnCoordinates => limits.max_physical_column_coordinate_cells = value,
            Self::Entries => limits.max_physical_entries = value,
            Self::RowOffsets => limits.max_csr_row_offsets = value,
        }
    }
}

fn stable_structure_bytes(frame: &OneSidedChartFrame) -> Vec<u8> {
    let plan = frame.plan();
    let mut bytes = Vec::new();
    push_usize(&mut bytes, plan.sector().arity());
    bytes.extend(
        plan.sector()
            .active_bits()
            .iter()
            .map(|&active| u8::from(active)),
    );
    push_usize(&mut bytes, frame.degree());
    push_shifts(&mut bytes, frame.offsets());
    push_shifts(&mut bytes, plan.columns());
    push_u32s(&mut bytes, plan.row_offsets());
    push_u32s(&mut bytes, plan.column_indices());
    push_usize(&mut bytes, plan.source_instances().len());
    for source in plan.source_instances() {
        let stable = source.stable_string();
        push_usize(&mut bytes, stable.len());
        bytes.extend_from_slice(stable.as_bytes());
    }
    bytes
}

fn push_shifts(bytes: &mut Vec<u8>, shifts: &[crate::identity::IntegralShift]) {
    push_usize(bytes, shifts.len());
    for shift in shifts {
        push_usize(bytes, shift.len());
        for &value in shift.values() {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
}

fn push_u32s(bytes: &mut Vec<u8>, values: &[u32]) {
    push_usize(bytes, values.len());
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}

fn push_usize(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(&u64::try_from(value).unwrap().to_le_bytes());
}

fn chart_frame_order(
    sector: &Mask,
    left: &crate::identity::IntegralShift,
    right: &crate::identity::IntegralShift,
) -> Ordering {
    let left_degree = left
        .values()
        .iter()
        .map(|value| value.unsigned_abs())
        .sum::<u64>();
    let right_degree = right
        .values()
        .iter()
        .map(|value| value.unsigned_abs())
        .sum::<u64>();
    left_degree.cmp(&right_degree).then_with(|| {
        left.values()
            .iter()
            .zip(right.values())
            .zip(sector.active_bits())
            .find_map(|((&left, &right), &active)| {
                assert!(if active { left >= 0 } else { left <= 0 });
                assert!(if active { right >= 0 } else { right <= 0 });
                let left = left.unsigned_abs();
                let right = right.unsigned_abs();
                (left != right).then(|| left.cmp(&right))
            })
            .unwrap_or(Ordering::Equal)
    })
}
