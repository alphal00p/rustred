use std::collections::BTreeSet;

use crate::family::IntegralKey;
use crate::sector::Mask;

use super::{
    CompletionGeometryError, CompletionGeometryLimits, LatticePoint, LeadingIdeal, SectorChart,
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

fn point<const N: usize>(coordinates: [u64; N]) -> LatticePoint {
    LatticePoint::try_new(coordinates).unwrap()
}
