use super::*;
use crate::sector::OrderingPolicy;

fn slices(sector: &[bool], degree: u64) -> Vec<LatticeBox> {
    RankScopeBudget::new(Default::default())
        .slices(sector, degree)
        .unwrap()
}

fn contains(boxes: &[LatticeBox], point: &[u64]) -> bool {
    boxes.iter().any(|cell| {
        point.len() == cell.arity()
            && point.iter().enumerate().all(|(axis, &value)| {
                value >= cell.lower()[axis] && cell.upper()[axis].is_none_or(|upper| value <= upper)
            })
    })
}

#[test]
fn degree_zero_fixes_only_inactive_axes_and_preserves_infinity() {
    let boxes = slices(&[true, false, true, false], 0);
    assert_eq!(boxes.len(), 1);
    assert_eq!(boxes[0].lower(), [0; 4]);
    assert_eq!(boxes[0].upper(), [None, Some(0), None, Some(0)]);
    assert!(contains(&boxes, &[u64::MAX, 0, u64::MAX, 0]));
    assert!(!contains(&boxes, &[0, 1, 0, 0]));
}

#[test]
fn all_active_scope_is_one_ray_even_at_maximal_bound() {
    let boxes = slices(&[true; 3], u64::MAX);
    assert_eq!(boxes.len(), 1);
    assert_eq!(boxes[0].upper(), [None; 3]);
    assert!(entry_contains(&[true; 3], &[i64::MAX; 3], 0).unwrap());
}

#[test]
fn simplex_enumeration_is_complete_disjoint_and_not_a_cube() {
    for degree in 0..=4 {
        let boxes = slices(&[false, true, false, false], degree);
        assert_eq!(
            boxes.len(),
            ((degree + 1) * (degree + 2) * (degree + 3) / 6) as usize
        );
        for a in 0..=degree + 1 {
            for b in 0..=degree + 1 {
                for c in 0..=degree + 1 {
                    let point = [a, u64::MAX, b, c];
                    let hits = boxes
                        .iter()
                        .filter(|cell| contains(std::slice::from_ref(*cell), &point))
                        .count();
                    assert_eq!(hits, usize::from(a + b + c <= degree));
                }
            }
        }
    }
    assert!(!contains(&slices(&[false; 2], 2), &[2, 2]));
}

#[test]
fn all_inactive_scope_has_only_finite_singletons() {
    let boxes = slices(&[false; 3], 2);
    assert_eq!(boxes.len(), 10);
    for cell in boxes {
        assert!(cell.upper().iter().all(Option::is_some));
        assert!(
            cell.lower()
                .iter()
                .zip(cell.upper())
                .all(|(&lower, &upper)| upper == Some(lower))
        );
    }
}

#[test]
fn permutation_changes_axes_not_scope_or_native_sized_powers() {
    let sector = [true, false, false];
    let swapped = [false, true, false];
    let original = slices(&sector, 3);
    let permuted = slices(&swapped, 3);
    for a in 0..=4 {
        for b in 0..=4 {
            assert_eq!(
                contains(&original, &[99, a, b]),
                contains(&permuted, &[a, 99, b])
            );
        }
    }
    assert_eq!(original, slices(&sector, 3));
    assert!(entry_contains(&sector, &[i64::MAX, -2, -1], 3).unwrap());
    assert!(entry_contains(&swapped, &[-2, i64::MAX, -1], 3).unwrap());
}

#[test]
fn entry_degree_is_a_wide_sum_over_all_negative_axes() {
    assert!(entry_contains(&[true, false], &[-1, -2], 3).unwrap());
    assert!(!entry_contains(&[true, false], &[-2, -2], 3).unwrap());
    assert!(!entry_contains(&[true, false], &[1, 1], 3).unwrap());
    assert!(entry_contains(&[false], &[i64::MIN], 1_u64 << 63).unwrap());
    assert!(!entry_contains(&[false], &[i64::MIN], (1_u64 << 63) - 1).unwrap());
    assert!(!entry_contains(&[false; 2], &[i64::MIN; 2], u64::MAX).unwrap());
    assert!(entry_contains(&[false], &[-40_000], 40_000).unwrap());
}

#[test]
fn malformed_arity_is_rejected_before_a_scope_claim() {
    assert!(entry_contains(&[], &[], 0).is_err());
    assert!(entry_contains(&[false], &[], 0).is_err());
    assert!(
        RankScopeBudget::new(Default::default())
            .slices(&[], 0)
            .is_err()
    );
    let limits = CompletionGeometryLimits {
        max_arity: 1,
        ..Default::default()
    };
    assert!(RankScopeBudget::new(limits).slices(&[false; 2], 0).is_err());
}

#[test]
fn count_and_coordinate_limits_admit_the_exact_boundary_only() {
    // k=1, n=2, D=2: three offsets; 3*(2*n+k)+(k-1)=15
    // coordinates, including temporary offset storage.
    let limits = CompletionGeometryLimits {
        max_requested_boxes: 3,
        max_requested_box_coordinate_cells: 15,
        max_split_operations: 43,
        ..Default::default()
    };
    let mut budget = RankScopeBudget::new(limits);
    assert_eq!(budget.slices(&[true, false], 2).unwrap().len(), 3);
    assert_eq!(
        (
            budget.boxes,
            budget.coordinate_cells,
            budget.structural_work
        ),
        (3, 15, 43)
    );
    for limits in [
        CompletionGeometryLimits {
            max_requested_boxes: 2,
            ..limits
        },
        CompletionGeometryLimits {
            max_requested_box_coordinate_cells: 14,
            ..limits
        },
        CompletionGeometryLimits {
            max_split_operations: 42,
            ..limits
        },
    ] {
        let mut budget = RankScopeBudget::new(limits);
        assert!(matches!(
            budget.slices(&[true, false], 2),
            Err(ArtifactError::ResourceLimit { .. })
        ));
        assert_eq!(budget.boxes, 0);
    }
}

#[test]
fn multiple_sectors_share_one_quota_and_failed_preflight_cannot_reset_it() {
    let limits = CompletionGeometryLimits {
        max_requested_boxes: 4,
        ..Default::default()
    };
    let mut budget = RankScopeBudget::new(limits);
    budget.slices(&[true, false], 2).unwrap();
    assert!(budget.slices(&[false, true], 1).is_err());
    assert_eq!(budget.boxes, 3);
    budget.slices(&[true, true], u64::MAX).unwrap();
    assert_eq!(budget.boxes, 4);
    assert!(budget.slices(&[true, true], 0).is_err());

    for limits in [
        CompletionGeometryLimits {
            max_requested_box_coordinate_cells: 29,
            ..Default::default()
        },
        CompletionGeometryLimits {
            max_split_operations: 85,
            ..Default::default()
        },
    ] {
        let mut budget = RankScopeBudget::new(limits);
        budget.slices(&[true, false], 2).unwrap();
        assert!(budget.slices(&[false, true], 2).is_err());
        assert_eq!(
            (
                budget.boxes,
                budget.coordinate_cells,
                budget.structural_work
            ),
            (3, 15, 43)
        );
    }
}

#[test]
fn extreme_simplex_counts_and_zero_allowances_fail_before_enumeration() {
    for (sector, degree) in [(&[false][..], u64::MAX), (&[false; 64][..], 64)] {
        let mut budget = RankScopeBudget::new(Default::default());
        assert!(matches!(
            budget.slices(sector, degree),
            Err(ArtifactError::ResourceCountOverflow { .. })
                | Err(ArtifactError::ResourceLimit { .. })
        ));
        assert_eq!(budget.boxes, 0);
    }
    let limits = CompletionGeometryLimits {
        max_requested_boxes: 0,
        ..Default::default()
    };
    assert!(RankScopeBudget::new(limits).slices(&[true], 0).is_err());

    let limits = CompletionGeometryLimits {
        max_requested_boxes: usize::MAX,
        max_requested_box_coordinate_cells: usize::MAX,
        max_split_operations: usize::MAX,
        ..Default::default()
    };
    let mut large_sector = [true; 4096];
    large_sector[0] = false;
    assert!(matches!(
        RankScopeBudget::new(limits).slices(&large_sector, (usize::MAX / 2) as u64),
        Err(ArtifactError::ResourceCountOverflow {
            resource: "rank-scope coordinate cells"
        })
    ));
}

#[test]
fn strictly_descending_rhs_can_escape_the_entry_rank_scope() {
    let parent = [3, 0];
    let child = [1, -1];
    for order in [
        OrderingPolicy::RustRedUnshiftedV1,
        OrderingPolicy::SpiredUncutV1,
    ] {
        order.prove_strict_descent(&parent, &child).unwrap();
    }
    assert!(entry_contains(&[true, false], &parent, 0).unwrap());
    assert!(!entry_contains(&[true, false], &child, 0).unwrap());
    assert!(entry_contains(&[true, false], &child, 1).unwrap());
    // This is only an escape witness: a future publisher must verify a larger
    // inductive envelope or reject, never truncate this nonzero contribution.
}
