use rustred::{
    CylindricalOrderingLimits, CylindricalParametricEliminationOrdering,
    CylindricalPreparePointError, CylindricalPreparePointLayer, CylindricalPreparePointLimits,
    IndexShift, IntegralOrderingPolicy, PartialIndexAssignment, SectorMask,
};
use std::collections::BTreeSet;

fn ordering(
    sector: &str,
    assignment: impl IntoIterator<Item = (usize, i64)>,
) -> CylindricalParametricEliminationOrdering {
    let sector = SectorMask::try_from_bit_string(sector).unwrap();
    let assignment =
        PartialIndexAssignment::try_new(assignment, sector.arity(), sector.arity()).unwrap();
    CylindricalParametricEliminationOrdering::try_new(
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        sector,
        assignment,
        CylindricalOrderingLimits::default(),
    )
    .unwrap()
}

#[test]
fn empty_assignment_is_a_fully_symbolic_unfiltered_start() {
    let ordering = ordering("10", []);
    let layer = CylindricalPreparePointLayer::compile(
        ordering.clone(),
        1,
        CylindricalPreparePointLimits::default(),
    )
    .unwrap();
    layer.replay().unwrap();
    let mut expected = [
        IndexShift::try_new([-1, 0], 2).unwrap(),
        IndexShift::try_new([0, -1], 2).unwrap(),
        IndexShift::try_new([0, 1], 2).unwrap(),
        IndexShift::try_new([1, 0], 2).unwrap(),
    ];
    expected.sort_by(|left, right| ordering.compare_shifts(left, right).unwrap());
    assert_eq!(layer.ordered_translations(), expected);
    assert_eq!(layer.stats().enumerated_offsets(), 4);
    assert_eq!(layer.stats().fixed_sector_checks(), 0);
    assert_eq!(layer.stats().rejected_fixed_sector_offsets(), 0);
}

#[test]
fn only_literal_fixed_coordinates_are_sector_filtered() {
    // n0=1 is fixed in the active half-line.  The (-1,0) offset crosses its
    // boundary and is rejected.  The free inactive coordinate is never
    // filtered, so both (0,-1) and (0,+1) survive.
    let layer = CylindricalPreparePointLayer::compile(
        ordering("10", [(0, 1)]),
        1,
        CylindricalPreparePointLimits::default(),
    )
    .unwrap();
    let values = layer
        .ordered_translations()
        .iter()
        .map(|shift| shift.values().to_vec())
        .collect::<Vec<_>>();
    assert!(!values.contains(&vec![-1, 0]));
    assert!(values.contains(&vec![0, -1]));
    assert!(values.contains(&vec![0, 1]));
    assert!(values.contains(&vec![1, 0]));
    assert_eq!(layer.stats().enumerated_offsets(), 4);
    assert_eq!(layer.stats().rejected_fixed_sector_offsets(), 1);
}

#[test]
fn exact_shell_has_no_inner_ball_points_and_replays_in_order() {
    let layer = CylindricalPreparePointLayer::compile(
        ordering("11", []),
        2,
        CylindricalPreparePointLimits::default(),
    )
    .unwrap();
    assert_eq!(layer.ordered_translations().len(), 8);
    assert!(layer.ordered_translations().iter().all(|shift| {
        shift
            .values()
            .iter()
            .map(|value| value.unsigned_abs())
            .sum::<u64>()
            == 2
    }));
    layer.replay().unwrap();
}

#[test]
fn depth_zero_and_full_assignment_obey_single_start_geometry() {
    let zero = CylindricalPreparePointLayer::compile(
        ordering("01", [(0, 0), (1, 1)]),
        0,
        CylindricalPreparePointLimits::default(),
    )
    .unwrap();
    assert_eq!(
        zero.ordered_translations(),
        [IndexShift::try_new([0, 0], 2).unwrap()]
    );

    let boundary = CylindricalPreparePointLayer::compile(
        ordering("01", [(0, 0), (1, 1)]),
        1,
        CylindricalPreparePointLimits::default(),
    )
    .unwrap();
    let values = boundary
        .ordered_translations()
        .iter()
        .map(|shift| shift.values().to_vec())
        .collect::<Vec<_>>();
    assert_eq!(values.len(), 2);
    assert!(values.contains(&vec![-1, 0]));
    assert!(values.contains(&vec![0, 1]));
}

#[test]
fn every_retained_resource_limit_fails_closed() {
    let base = CylindricalPreparePointLayer::compile(
        ordering("11", []),
        2,
        CylindricalPreparePointLimits::default(),
    )
    .unwrap();
    let stats = base.stats();

    let mut limits = CylindricalPreparePointLimits::default();
    limits.max_retained_points = stats.retained_points() - 1;
    assert!(matches!(
        CylindricalPreparePointLayer::compile(ordering("11", []), 2, limits),
        Err(CylindricalPreparePointError::ResourceLimit {
            resource: "retained prepare points",
            ..
        })
    ));

    let mut limits = CylindricalPreparePointLimits::default();
    limits.max_order_key_components = stats.order_key_components() - 1;
    assert!(matches!(
        CylindricalPreparePointLayer::compile(ordering("11", []), 2, limits),
        Err(CylindricalPreparePointError::ResourceLimit {
            resource: "prepare-point order-key components",
            ..
        })
    ));

    let mut limits = CylindricalPreparePointLimits::default();
    limits.max_order_comparisons = stats.order_comparisons() - 1;
    assert!(matches!(
        CylindricalPreparePointLayer::compile(ordering("11", []), 2, limits),
        Err(CylindricalPreparePointError::ResourceLimit {
            resource: "prepare-point order comparisons",
            ..
        })
    ));
}

#[test]
fn all_pre_sort_limits_are_checked_one_below() {
    let base = CylindricalPreparePointLayer::compile(
        ordering("10", [(0, 1)]),
        2,
        CylindricalPreparePointLimits::default(),
    )
    .unwrap();
    let stats = base.stats();
    let failures = [
        ("prepare-point enumeration steps", stats.enumeration_steps()),
        (
            "enumerated prepare-point offsets",
            stats.enumerated_offsets(),
        ),
        (
            "enumerated prepare-point components",
            stats.enumerated_components(),
        ),
        (
            "fixed-coordinate sector checks",
            stats.fixed_sector_checks(),
        ),
        (
            "retained prepare-point components",
            stats.retained_components(),
        ),
    ];
    for (resource, observed) in failures {
        assert!(observed > 0);
        let mut limits = CylindricalPreparePointLimits::default();
        match resource {
            "prepare-point enumeration steps" => limits.max_enumeration_steps = observed - 1,
            "enumerated prepare-point offsets" => limits.max_enumerated_offsets = observed - 1,
            "enumerated prepare-point components" => {
                limits.max_enumerated_components = observed - 1
            }
            "fixed-coordinate sector checks" => limits.max_fixed_sector_checks = observed - 1,
            "retained prepare-point components" => limits.max_retained_components = observed - 1,
            _ => unreachable!(),
        }
        assert!(matches!(
            CylindricalPreparePointLayer::compile(ordering("10", [(0, 1)]), 2, limits),
            Err(CylindricalPreparePointError::ResourceLimit { resource: actual, .. })
                if actual == resource
        ));
    }

    let mut limits = CylindricalPreparePointLimits::default();
    limits.max_depth = 1;
    assert!(matches!(
        CylindricalPreparePointLayer::compile(ordering("10", [(0, 1)]), 2, limits),
        Err(CylindricalPreparePointError::DepthTooLarge {
            requested: 2,
            limit: 1
        })
    ));
}

#[test]
fn exhaustive_small_shells_match_an_independent_cube_oracle() {
    fn cube(position: usize, depth: i64, current: &mut [i64], output: &mut Vec<Vec<i64>>) {
        if position == current.len() {
            if current
                .iter()
                .map(|value| value.unsigned_abs())
                .sum::<u64>()
                == depth as u64
            {
                output.push(current.to_vec());
            }
            return;
        }
        for value in -depth..=depth {
            current[position] = value;
            cube(position + 1, depth, current, output);
        }
    }

    for arity in 1..=3usize {
        for sector_bits in 0..(1usize << arity) {
            let sector = (0..arity)
                .map(|position| (sector_bits & (1 << position)) != 0)
                .collect::<Vec<_>>();
            let sector_string = sector
                .iter()
                .map(|active| if *active { '1' } else { '0' })
                .collect::<String>();
            for fixed_mask in 0..(1usize << arity) {
                let assignment = (0..arity)
                    .filter(|position| (fixed_mask & (1 << position)) != 0)
                    .map(|position| (position, if sector[position] { 1 } else { 0 }))
                    .collect::<Vec<_>>();
                let ordering = ordering(&sector_string, assignment.clone());
                for depth in 0..=3usize {
                    let layer = CylindricalPreparePointLayer::compile(
                        ordering.clone(),
                        depth,
                        CylindricalPreparePointLimits::default(),
                    )
                    .unwrap();
                    let mut oracle = Vec::new();
                    cube(0, depth as i64, &mut vec![0; arity], &mut oracle);
                    oracle.retain(|offset| {
                        assignment.iter().all(|&(position, value)| {
                            (value + offset[position] >= 1) == sector[position]
                        })
                    });
                    let expected = oracle.into_iter().collect::<BTreeSet<_>>();
                    let actual = layer
                        .ordered_translations()
                        .iter()
                        .map(|shift| shift.values().to_vec())
                        .collect::<BTreeSet<_>>();
                    assert_eq!(actual, expected);
                    assert!(layer.ordered_translations().windows(2).all(|pair| {
                        ordering.compare_shifts(&pair[0], &pair[1]).unwrap().is_lt()
                    }));
                }
            }
        }
    }
}

#[test]
fn direct_shell_iterator_avoids_radius_sized_dead_work_at_arity_one() {
    let layer = CylindricalPreparePointLayer::compile(
        ordering("1", []),
        1_000_000,
        CylindricalPreparePointLimits {
            max_depth: 1_000_000,
            max_enumeration_steps: 1,
            max_enumerated_offsets: 2,
            max_enumerated_components: 2,
            max_fixed_sector_checks: 0,
            max_retained_points: 2,
            max_retained_components: 2,
            max_order_key_components: 16,
            max_order_comparisons: 1,
        },
    )
    .unwrap();
    assert_eq!(layer.stats().enumeration_steps(), 1);
    assert_eq!(layer.stats().enumerated_offsets(), 2);
}

#[test]
fn fixed_coordinate_addition_overflow_is_typed() {
    let active = CylindricalParametricEliminationOrdering::try_new(
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        SectorMask::try_from_bit_string("1").unwrap(),
        PartialIndexAssignment::try_new([(0, i64::MAX)], 1, 1).unwrap(),
        CylindricalOrderingLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        CylindricalPreparePointLayer::compile(active, 1, CylindricalPreparePointLimits::default()),
        Err(CylindricalPreparePointError::FixedIndexOverflow {
            position: 0,
            value: i64::MAX,
            displacement: 1,
        })
    ));

    let inactive = CylindricalParametricEliminationOrdering::try_new(
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        SectorMask::try_from_bit_string("0").unwrap(),
        PartialIndexAssignment::try_new([(0, i64::MIN)], 1, 1).unwrap(),
        CylindricalOrderingLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        CylindricalPreparePointLayer::compile(
            inactive,
            1,
            CylindricalPreparePointLimits::default()
        ),
        Err(CylindricalPreparePointError::FixedIndexOverflow {
            position: 0,
            value: i64::MIN,
            displacement: -1,
        })
    ));
}
