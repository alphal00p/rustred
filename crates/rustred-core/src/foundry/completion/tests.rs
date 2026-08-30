use std::collections::BTreeSet;

use crate::family::IntegralKey;
use crate::sector::Mask;

use super::{
    BoxCover, CompletionGeometryError, CompletionGeometryLimits, LatticeBox, LatticeCardinality,
    LatticePoint, LeadingIdeal, SectorChart,
};

#[test]
fn sector_chart_is_exact_at_every_i64_endpoint() {
    let chart = SectorChart::new(Mask::try_new([true, false, true, false]).unwrap());
    assert_eq!(chart.sector().active_bits(), [true, false, true, false]);
    let integral = IntegralKey::try_new([i64::MAX, i64::MIN, 1, 0]).unwrap();
    let point = chart.to_lattice(&integral).unwrap();
    assert_eq!(
        point.coordinates(),
        [i64::MAX as u64 - 1, 1_u64 << 63, 0, 0]
    );
    let carrier = chart.carrier_box().unwrap();
    assert!(carrier.contains(&point));
    assert_eq!(carrier.varying_dimension(), 4);
    assert_eq!(chart.to_integral(&point).unwrap(), integral);

    let active_overflow = LatticePoint::try_new([i64::MAX as u64, 0, 0, 0]).unwrap();
    assert_eq!(
        chart.to_integral(&active_overflow),
        Err(CompletionGeometryError::CoordinateNotRepresentable {
            position: 0,
            coordinate: i64::MAX as u64,
            active: true,
        })
    );
    let inactive_overflow = LatticePoint::try_new([0, (1_u64 << 63) + 1, 0, 0]).unwrap();
    assert_eq!(
        chart.to_integral(&inactive_overflow),
        Err(CompletionGeometryError::CoordinateNotRepresentable {
            position: 1,
            coordinate: (1_u64 << 63) + 1,
            active: false,
        })
    );
}

#[test]
fn sector_chart_rejects_wrong_signs_and_arities_without_reclassification() {
    let chart = SectorChart::new(Mask::try_new([true, false]).unwrap());
    assert_eq!(
        chart.to_lattice(&IntegralKey::try_new([0, 0]).unwrap()),
        Err(CompletionGeometryError::IntegralOutsideSector {
            position: 0,
            power: 0,
            active: true,
        })
    );
    assert_eq!(
        chart.to_lattice(&IntegralKey::try_new([1, 1]).unwrap()),
        Err(CompletionGeometryError::IntegralOutsideSector {
            position: 1,
            power: 1,
            active: false,
        })
    );
    assert_eq!(
        chart.to_integral(&LatticePoint::try_new([0]).unwrap()),
        Err(CompletionGeometryError::WrongArity {
            object: "lattice point",
            expected: 2,
            actual: 1,
        })
    );
}

#[test]
fn minimal_generators_and_unbounded_complement_are_exact() {
    let ideal = LeadingIdeal::try_new(
        2,
        [point([1, 1]), point([2, 2]), point([1, 1]), point([3, 0])],
        CompletionGeometryLimits::default(),
    )
    .unwrap();
    assert_eq!(ideal.arity(), 2);
    assert_eq!(
        ideal.generators(),
        [point([1, 1]), point([3, 0])].as_slice()
    );
    let uncovered = ideal.uncovered_partition().unwrap();
    assert!(!uncovered.is_finite());
    assert!(uncovered.split_operations() > 0);
    assert_eq!(uncovered.boxes().len(), 2);
    assert_exact_on_cube(&ideal, 7, &uncovered);
    assert!(uncovered.containing_box(&point([0, 100])).is_some());
    assert!(uncovered.containing_box(&point([2, 0])).is_some());
    assert!(uncovered.containing_box(&point([3, 0])).is_none());
    assert!(uncovered.containing_box(&point([1, 1])).is_none());
}

#[test]
fn pure_power_leaders_leave_one_finite_staircase_box() {
    let ideal = LeadingIdeal::try_new(
        2,
        [point([2, 0]), point([0, 3])],
        CompletionGeometryLimits::default(),
    )
    .unwrap();
    let uncovered = ideal.uncovered_partition().unwrap();
    assert!(uncovered.is_finite());
    assert_eq!(uncovered.boxes().len(), 1);
    assert_eq!(uncovered.boxes()[0].lower(), [0, 0]);
    assert_eq!(uncovered.boxes()[0].upper(), [Some(1), Some(2)]);
    assert_exact_on_cube(&ideal, 7, &uncovered);
}

#[test]
fn empty_and_unit_leading_ideals_have_exact_extreme_complements() {
    let empty = LeadingIdeal::try_new(3, [], CompletionGeometryLimits::default()).unwrap();
    let all_uncovered = empty.uncovered_partition().unwrap();
    assert_eq!(all_uncovered.boxes().len(), 1);
    assert_eq!(all_uncovered.boxes()[0].free_dimension(), 3);

    let unit =
        LeadingIdeal::try_new(3, [point([0, 0, 0])], CompletionGeometryLimits::default()).unwrap();
    let none_uncovered = unit.uncovered_partition().unwrap();
    assert!(none_uncovered.is_empty());
}

#[test]
fn resource_limits_fail_before_retaining_the_excess_box_or_generator() {
    let mut limits = CompletionGeometryLimits::default();
    limits.max_requested_generator_coordinate_cells = 3;
    assert_eq!(
        LeadingIdeal::try_new(2, [point([1, 0]), point([0, 1])], limits),
        Err(CompletionGeometryError::ResourceLimit {
            resource: "requested leading-generator coordinate cells",
            requested: 4,
            limit: 3,
        })
    );

    let mut limits = CompletionGeometryLimits::default();
    limits.max_uncovered_boxes = 1;
    let ideal = LeadingIdeal::try_new(2, [point([1, 1])], limits).unwrap();
    assert_eq!(
        ideal.uncovered_partition(),
        Err(CompletionGeometryError::ResourceLimit {
            resource: "uncovered lattice boxes",
            requested: 2,
            limit: 1,
        })
    );
}

#[test]
fn arbitrary_overlapping_box_union_has_an_exact_disjoint_complement() {
    let cover = BoxCover::try_new(
        2,
        [
            lattice_box([0, 0], [Some(2), Some(1)]),
            lattice_box([1, 1], [Some(3), Some(3)]),
            lattice_box([1, 1], [Some(3), Some(3)]),
        ],
        CompletionGeometryLimits::default(),
    )
    .unwrap();
    assert_eq!(cover.boxes().len(), 2);
    let uncovered = cover.uncovered_partition().unwrap();
    assert_eq!(
        uncovered.try_cardinality(10_000).unwrap(),
        LatticeCardinality::Infinite
    );
    assert_exact_box_cover_on_square(&cover, 8, &uncovered);
}

#[test]
fn arbitrary_box_union_can_leave_a_small_finite_terminal_budget() {
    let cover = BoxCover::try_new(
        2,
        [
            lattice_box([2, 0], [None, None]),
            lattice_box([0, 3], [Some(1), None]),
        ],
        CompletionGeometryLimits::default(),
    )
    .unwrap();
    let uncovered = cover.uncovered_partition().unwrap();
    assert!(uncovered.is_finite());
    assert_eq!(
        uncovered.try_cardinality(6).unwrap(),
        LatticeCardinality::Finite(6)
    );
    assert_eq!(
        uncovered.try_cardinality(5),
        Err(CompletionGeometryError::ResourceLimit {
            resource: "finite uncovered lattice points",
            requested: 6,
            limit: 5,
        })
    );
    assert_exact_box_cover_on_square(&cover, 8, &uncovered);
}

#[test]
fn arbitrary_box_construction_and_cover_limits_fail_closed() {
    assert_eq!(
        LatticeBox::try_new([0, 1], [Some(2)]),
        Err(CompletionGeometryError::WrongArity {
            object: "lattice-box upper endpoints",
            expected: 2,
            actual: 1,
        })
    );
    assert_eq!(
        LatticeBox::try_new([0, 3], [Some(1), Some(2)]),
        Err(CompletionGeometryError::InvalidBoxBounds {
            position: 1,
            lower: 3,
            upper: 2,
        })
    );

    let mut limits = CompletionGeometryLimits::default();
    limits.max_requested_boxes = 1;
    assert_eq!(
        BoxCover::try_new(
            2,
            [
                lattice_box([0, 0], [Some(0), Some(0)]),
                lattice_box([1, 1], [Some(1), Some(1)]),
            ],
            limits,
        ),
        Err(CompletionGeometryError::ResourceLimit {
            resource: "requested structural cover boxes",
            requested: 2,
            limit: 1,
        })
    );
    limits.max_requested_box_coordinate_cells = 4;
    assert!(BoxCover::try_new(2, [lattice_box([0, 0], [Some(0), Some(0)])], limits,).is_ok());

    let mut limits = CompletionGeometryLimits::default();
    limits.max_requested_box_coordinate_cells = 3;
    assert_eq!(
        BoxCover::try_new(2, [lattice_box([0, 0], [Some(0), Some(0)])], limits,),
        Err(CompletionGeometryError::ResourceLimit {
            resource: "requested structural-cover coordinate cells",
            requested: 4,
            limit: 3,
        })
    );

    let mut limits = CompletionGeometryLimits::default();
    limits.max_uncovered_boxes = 0;
    let empty_cover = BoxCover::try_new(2, [], limits).unwrap();
    assert_eq!(
        empty_cover.uncovered_partition(),
        Err(CompletionGeometryError::ResourceLimit {
            resource: "uncovered lattice boxes",
            requested: 1,
            limit: 0,
        })
    );
    let empty_ideal = LeadingIdeal::try_new(2, [], limits).unwrap();
    assert_eq!(
        empty_ideal.uncovered_partition(),
        Err(CompletionGeometryError::ResourceLimit {
            resource: "uncovered lattice boxes",
            requested: 1,
            limit: 0,
        })
    );

    let mut limits = CompletionGeometryLimits::default();
    limits.max_uncovered_box_coordinate_cells = 3;
    let empty_cover = BoxCover::try_new(2, [], limits).unwrap();
    assert_eq!(
        empty_cover.uncovered_partition(),
        Err(CompletionGeometryError::ResourceLimit {
            resource: "uncovered-box coordinate cells",
            requested: 4,
            limit: 3,
        })
    );
    limits.max_uncovered_box_coordinate_cells = 4;
    assert_eq!(
        BoxCover::try_new(2, [], limits)
            .unwrap()
            .uncovered_partition()
            .unwrap()
            .boxes()
            .len(),
        1
    );
}

#[test]
fn a_full_arbitrary_box_cover_has_zero_cardinality() {
    let cover = BoxCover::try_new(
        3,
        [lattice_box([0, 0, 0], [None, None, None])],
        CompletionGeometryLimits::default(),
    )
    .unwrap();
    let uncovered = cover.uncovered_partition().unwrap();
    assert!(uncovered.is_empty());
    assert_eq!(
        uncovered.try_cardinality(0).unwrap(),
        LatticeCardinality::Finite(0)
    );
}

#[test]
fn arbitrary_box_subtraction_is_exact_inside_a_finite_carrier() {
    let cover = BoxCover::try_new(
        2,
        [lattice_box([1, 1], [Some(2), Some(2)])],
        CompletionGeometryLimits::default(),
    )
    .unwrap();
    let uncovered = cover
        .uncovered_within(lattice_box([0, 0], [Some(3), Some(3)]))
        .unwrap();
    assert_eq!(
        uncovered.try_cardinality(12).unwrap(),
        LatticeCardinality::Finite(12)
    );
    assert_exact_box_cover_on_square(&cover, 4, &uncovered);
}

#[test]
fn every_pair_of_small_mixed_boxes_has_the_exact_carrier_complement() {
    let intervals = [
        (0, Some(0)),
        (0, Some(1)),
        (0, Some(2)),
        (0, Some(3)),
        (1, Some(1)),
        (1, Some(2)),
        (1, Some(3)),
        (2, Some(2)),
        (2, Some(3)),
        (3, Some(3)),
        (0, None),
        (1, None),
        (2, None),
    ];
    for &(first_lower, first_upper) in &intervals {
        for &(second_lower, second_upper) in &intervals {
            for &(third_lower, third_upper) in &intervals {
                for &(fourth_lower, fourth_upper) in &intervals {
                    let cover = BoxCover::try_new(
                        2,
                        [
                            lattice_box([first_lower, second_lower], [first_upper, second_upper]),
                            lattice_box([third_lower, fourth_lower], [third_upper, fourth_upper]),
                        ],
                        CompletionGeometryLimits::default(),
                    )
                    .unwrap();
                    let uncovered = cover
                        .uncovered_within(lattice_box([0, 0], [Some(2), Some(2)]))
                        .unwrap();

                    let mut expected_cardinality = 0usize;
                    for first in 0..=2 {
                        for second in 0..=2 {
                            let candidate = point([first, second]);
                            let containing = uncovered
                                .boxes()
                                .iter()
                                .filter(|cell| cell.contains(&candidate))
                                .count();
                            let covered = cover.covers(&candidate).unwrap();
                            assert_eq!(containing, usize::from(!covered));
                            expected_cardinality += usize::from(!covered);
                        }
                    }
                    assert_eq!(
                        uncovered.try_cardinality(9).unwrap(),
                        LatticeCardinality::Finite(expected_cardinality)
                    );
                }
            }
        }
    }
}

#[test]
fn arbitrary_box_complements_are_exact_for_nonzero_origins_and_three_dimensions() {
    let offset_cover = BoxCover::try_new(
        2,
        [lattice_box([3, 4], [Some(4), Some(5)])],
        CompletionGeometryLimits::default(),
    )
    .unwrap();
    let offset_uncovered = offset_cover
        .uncovered_within(lattice_box([2, 3], [Some(5), Some(6)]))
        .unwrap();
    assert_eq!(
        offset_uncovered.try_cardinality(12).unwrap(),
        LatticeCardinality::Finite(12)
    );
    for first in 2..=5 {
        for second in 3..=6 {
            let candidate = point([first, second]);
            let containing = offset_uncovered
                .boxes()
                .iter()
                .filter(|cell| cell.contains(&candidate))
                .count();
            assert_eq!(
                containing,
                usize::from(!offset_cover.covers(&candidate).unwrap())
            );
        }
    }

    let three_dimensional_cover = BoxCover::try_new(
        3,
        [
            lattice_box([0, 1, 0], [Some(1), Some(2), Some(0)]),
            lattice_box([2, 0, 1], [Some(2), Some(2), Some(2)]),
        ],
        CompletionGeometryLimits::default(),
    )
    .unwrap();
    let three_dimensional_uncovered = three_dimensional_cover
        .uncovered_within(lattice_box([0, 0, 0], [Some(2), Some(2), Some(2)]))
        .unwrap();
    assert_eq!(
        three_dimensional_uncovered.try_cardinality(17).unwrap(),
        LatticeCardinality::Finite(17)
    );
    for first in 0..=2 {
        for second in 0..=2 {
            for third in 0..=2 {
                let candidate = point([first, second, third]);
                let containing = three_dimensional_uncovered
                    .boxes()
                    .iter()
                    .filter(|cell| cell.contains(&candidate))
                    .count();
                assert_eq!(
                    containing,
                    usize::from(!three_dimensional_cover.covers(&candidate).unwrap())
                );
            }
        }
    }
}

#[test]
fn endpoint_and_cardinality_overflow_fail_closed() {
    let maximal_finite_cover = BoxCover::try_new(
        1,
        [lattice_box([0], [Some(u64::MAX)])],
        CompletionGeometryLimits::default(),
    )
    .unwrap();
    assert_eq!(
        maximal_finite_cover.uncovered_partition(),
        Err(CompletionGeometryError::ResourceCountOverflow {
            resource: "box-intersection successor coordinate",
        })
    );

    if usize::BITS == 64 {
        let width = 1_u64 << 32;
        let mut product_boxes = super::model::try_vec("product-overflow boxes", 1).unwrap();
        product_boxes.push(lattice_box([0, 0], [Some(width - 1), Some(width - 1)]));
        let product_overflow = super::UncoveredPartition::new(product_boxes, 0);
        assert_eq!(
            product_overflow.try_cardinality(usize::MAX),
            Err(CompletionGeometryError::ResourceCountOverflow {
                resource: "finite uncovered lattice points",
            })
        );

        let half = 1_u64 << 63;
        let mut total_boxes = super::model::try_vec("total-overflow boxes", 2).unwrap();
        total_boxes.push(lattice_box([0], [Some(half - 1)]));
        total_boxes.push(lattice_box([half], [Some(u64::MAX)]));
        let total_overflow = super::UncoveredPartition::new(total_boxes, 0);
        assert_eq!(
            total_overflow.try_cardinality(usize::MAX),
            Err(CompletionGeometryError::ResourceCountOverflow {
                resource: "finite uncovered lattice points",
            })
        );
    }
}

fn assert_exact_on_cube(ideal: &LeadingIdeal, side: u64, uncovered: &super::UncoveredPartition) {
    let mut represented = BTreeSet::new();
    for first in 0..side {
        for second in 0..side {
            let candidate = point([first, second]);
            let containing = uncovered
                .boxes()
                .iter()
                .filter(|cell| cell.contains(&candidate))
                .count();
            assert_eq!(containing, usize::from(!ideal.covers(&candidate).unwrap()));
            if containing == 1 {
                assert!(represented.insert((first, second)));
            }
        }
    }
}

fn assert_exact_box_cover_on_square(
    cover: &BoxCover,
    side: u64,
    uncovered: &super::UncoveredPartition,
) {
    for first in 0..side {
        for second in 0..side {
            let candidate = point([first, second]);
            let containing = uncovered
                .boxes()
                .iter()
                .filter(|cell| cell.contains(&candidate))
                .count();
            assert_eq!(containing, usize::from(!cover.covers(&candidate).unwrap()));
        }
    }
}

fn point<const N: usize>(coordinates: [u64; N]) -> LatticePoint {
    LatticePoint::try_new(coordinates).unwrap()
}

fn lattice_box<const N: usize>(lower: [u64; N], upper: [Option<u64>; N]) -> LatticeBox {
    LatticeBox::try_new(lower, upper).unwrap()
}
