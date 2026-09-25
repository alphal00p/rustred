use super::*;
use crate::solver::candidate_reduction::owners::domains::applied::geometry;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, Ordering};

fn policy(max: usize) -> OwnerAppliedCellRefinement {
    OwnerAppliedCellRefinement::SingleFiniteAxis {
        max_cardinality: NonZeroUsize::new(max).unwrap(),
    }
}

#[test]
fn application_refinement_exact_partition_is_axis_and_owner_independent() {
    for mask in 0..8 {
        let owner = std::array::from_fn::<_, 3, _>(|axis| mask & (1 << axis) != 0);
        for axis in 0..3 {
            for width in 2..=4 {
                let lower = [2, 1, 3];
                let mut upper = lower.map(Some);
                upper[axis] = Some(lower[axis] + width - 1);
                let source = LatticeBox::try_new(lower, upper).unwrap();
                let cancel = AtomicBool::new(false);
                let mut budget = Budget {
                    limits: OwnerAppliedLimits {
                        cell_refinement: policy(4),
                        ..Default::default()
                    },
                    stats: OwnerAppliedStats {
                        boundary_cells: 7,
                        ..Default::default()
                    },
                    cancel: &cancel,
                };
                let mut plan = Cells::new(&source, &owner, 1, &mut budget)
                    .unwrap()
                    .unwrap();
                let mut values = Vec::new();
                while let Some(cell) = plan.next(&mut budget).unwrap() {
                    values.push(cell.lower()[axis]);
                    for coordinate in 0..3 {
                        assert_eq!(cell.upper()[coordinate], Some(cell.lower()[coordinate]));
                        if coordinate != axis {
                            assert_eq!(cell.lower()[coordinate], lower[coordinate]);
                        }
                    }
                }
                assert_eq!(
                    values,
                    (lower[axis]..=upper[axis].unwrap()).collect::<Vec<_>>()
                );
                assert_eq!(budget.stats.application_refinement_steps, 1);
                assert_eq!(budget.stats.application_refinement_cells, width as usize);
                assert_eq!(budget.stats.boundary_cells, 7 + width as usize - 1);
            }
        }
    }
}

#[test]
fn application_refinement_policy_misses_are_unchanged_before_work() {
    let cancel = AtomicBool::new(false);
    for (owner, lower, upper, policy) in [
        (
            [true, false],
            [0, 0],
            [Some(1), Some(0)],
            OwnerAppliedCellRefinement::Off,
        ),
        ([true, false], [0, 0], [Some(0), Some(0)], policy(2)),
        ([true, false], [0, 0], [Some(1), Some(1)], policy(2)),
        ([true, false], [0, 0], [Some(1), None], policy(2)),
        ([true, false], [0, 0], [Some(2), Some(0)], policy(2)),
        ([true, false], [0, 0], [Some(1), Some(0)], policy(1)),
        // Active local i64::MAX cannot become physical index i64::MAX + 1.
        (
            [true, false],
            [i64::MAX as u64 - 1, 0],
            [Some(i64::MAX as u64), Some(0)],
            policy(2),
        ),
        // Inactive negative endpoint below i64::MIN is likewise ineligible.
        (
            [false, true],
            [i64::MAX as u64 + 1, 0],
            [Some(i64::MAX as u64 + 2), Some(0)],
            policy(2),
        ),
        (
            [false, true],
            [0, 0],
            [Some(u64::MAX), Some(0)],
            policy(usize::MAX),
        ),
    ] {
        let source = LatticeBox::try_new(lower, upper).unwrap();
        let mut budget = Budget {
            limits: OwnerAppliedLimits {
                cell_refinement: policy,
                ..Default::default()
            },
            stats: OwnerAppliedStats {
                boundary_cells: 1,
                ..Default::default()
            },
            cancel: &cancel,
        };
        let before = budget.stats;
        assert!(
            Cells::new(&source, &owner, 1, &mut budget)
                .unwrap()
                .is_none()
        );
        assert_eq!(budget.stats, before);
    }
    // The representable negative boundary itself remains eligible.
    let lower = [i64::MAX as u64, 0];
    let source = LatticeBox::try_new(lower, [Some(i64::MAX as u64 + 1), Some(0)]).unwrap();
    let mut budget = Budget {
        limits: OwnerAppliedLimits {
            cell_refinement: policy(2),
            ..Default::default()
        },
        stats: OwnerAppliedStats {
            boundary_cells: 1,
            ..Default::default()
        },
        cancel: &cancel,
    };
    let mut plan = Cells::new(&source, &[false, true], 1, &mut budget)
        .unwrap()
        .unwrap();
    let first = plan.next(&mut budget).unwrap().unwrap();
    let second = plan.next(&mut budget).unwrap().unwrap();
    assert_eq!(
        geometry::fixed(&first, &[false, true], None).unwrap()[0].1,
        -i64::MAX
    );
    assert_eq!(
        geometry::fixed(&second, &[false, true], None).unwrap()[0].1,
        i64::MIN
    );
    assert!(plan.next(&mut budget).unwrap().is_none());
}

#[test]
fn application_refinement_preflights_whole_partition_and_extra_live_scratch() {
    let source = LatticeBox::try_new([0, 0], [Some(1), Some(0)]).unwrap();
    let cancel = AtomicBool::new(false);
    let base = OwnerAppliedLimits {
        cell_refinement: policy(2),
        ..Default::default()
    };
    for (limits, expected) in [
        (
            OwnerAppliedLimits {
                max_boundary_cells: 1,
                ..base
            },
            "boundary cells",
        ),
        (
            OwnerAppliedLimits {
                max_scratch_boxes: 13,
                ..base
            },
            "scratch boxes",
        ),
        (
            OwnerAppliedLimits {
                max_scratch_coordinate_cells: 55,
                ..base
            },
            "scratch coordinate cells",
        ),
    ] {
        let mut budget = Budget {
            limits,
            stats: OwnerAppliedStats {
                boundary_cells: 1,
                ..Default::default()
            },
            cancel: &cancel,
        };
        let before = budget.stats;
        assert!(
            matches!(Cells::new(&source, &[true, false], 1, &mut budget),
            Err(OwnerAppliedFailure::ResourceLimit { resource, .. }) if resource == expected)
        );
        assert_eq!(budget.stats, before);
    }
    let mut budget = Budget {
        limits: base,
        stats: OwnerAppliedStats {
            boundary_cells: usize::MAX,
            ..Default::default()
        },
        cancel: &cancel,
    };
    assert!(matches!(
        Cells::new(&source, &[true, false], 1, &mut budget),
        Err(OwnerAppliedFailure::CountOverflow {
            resource: "boundary cells"
        })
    ));
}

#[test]
fn application_refinement_cancellation_preserves_only_actual_visited_cells() {
    let source = LatticeBox::try_new([0, 0], [Some(1), Some(0)]).unwrap();
    let cancel = AtomicBool::new(true);
    let mut budget = Budget {
        limits: OwnerAppliedLimits {
            cell_refinement: policy(2),
            ..Default::default()
        },
        stats: OwnerAppliedStats {
            boundary_cells: 1,
            ..Default::default()
        },
        cancel: &cancel,
    };
    assert!(matches!(
        Cells::new(&source, &[true, false], 1, &mut budget),
        Err(OwnerAppliedFailure::Cancelled)
    ));
    assert_eq!(budget.stats.application_refinement_steps, 0);
    cancel.store(false, Ordering::Release);
    let mut plan = Cells::new(&source, &[true, false], 1, &mut budget)
        .unwrap()
        .unwrap();
    assert!(plan.next(&mut budget).unwrap().is_some());
    cancel.store(true, Ordering::Release);
    assert!(matches!(
        plan.next(&mut budget),
        Err(OwnerAppliedFailure::Cancelled)
    ));
    assert_eq!(budget.stats.application_refinement_steps, 1);
    assert_eq!(budget.stats.application_refinement_cells, 1);
    assert_eq!(budget.stats.boundary_cells, 1);
}

#[test]
fn application_refinement_children_retain_rank_and_power_intersections() {
    let source = LatticeBox::try_new([0, 0], [Some(0), Some(3)]).unwrap();
    let cancel = AtomicBool::new(false);
    let mut budget = Budget {
        limits: OwnerAppliedLimits {
            cell_refinement: policy(4),
            ..Default::default()
        },
        stats: OwnerAppliedStats {
            boundary_cells: 1,
            ..Default::default()
        },
        cancel: &cancel,
    };
    let mut plan = Cells::new(&source, &[true, false], 1, &mut budget)
        .unwrap()
        .unwrap();
    let powers = crate::solver::DomainPowerBounds {
        max_positive_power: Some(1),
        min_power_difference: Some(0),
        max_power_difference: Some(1),
    };
    let mut live = Vec::new();
    while let Some(child) = plan.next(&mut budget).unwrap() {
        if let Some((cell, rank)) =
            geometry::normalize(child, &[true, false], Some(2), powers, &mut budget).unwrap()
        {
            assert!(rank.unwrap() <= 2);
            live.push(cell.lower()[1]);
        }
    }
    assert_eq!(live, [0, 1]);
    assert_eq!(budget.stats.application_refinement_cells, 4);
    assert_eq!(budget.stats.boundary_cells, 4);
    assert_eq!(budget.stats.correlation_empty_cells, 1);
}

#[test]
fn application_refinement_reserves_child_beyond_retained_sign_partition_peak() {
    let source = LatticeBox::try_new([1, 0], [Some(2), Some(0)]).unwrap();
    let cancel = AtomicBool::new(false);
    for sign_cells in [1, 2, 4] {
        let old_boxes = sign_cells + 12;
        for (boxes, coordinates, expected) in [
            (old_boxes, 4 * (old_boxes + 1), "scratch boxes"),
            (old_boxes + 1, 4 * old_boxes, "scratch coordinate cells"),
        ] {
            let mut budget = Budget {
                limits: OwnerAppliedLimits {
                    cell_refinement: policy(2),
                    max_scratch_boxes: boxes,
                    max_scratch_coordinate_cells: coordinates,
                    ..Default::default()
                },
                stats: OwnerAppliedStats {
                    boundary_cells: 1,
                    ..Default::default()
                },
                cancel: &cancel,
            };
            assert!(
                matches!(Cells::new(&source, &[true, false], sign_cells, &mut budget),
                Err(OwnerAppliedFailure::ResourceLimit { resource, .. }) if resource == expected)
            );
            assert_eq!(budget.stats.application_refinement_steps, 0);
        }
    }
}
