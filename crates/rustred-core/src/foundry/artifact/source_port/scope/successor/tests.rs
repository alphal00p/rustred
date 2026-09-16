use super::*;
use crate::sector::OrderingPolicy;

fn cell<const N: usize>(lower: [u64; N], upper: [Option<u64>; N]) -> LatticeBox {
    LatticeBox::try_new(lower, upper).unwrap()
}

fn contains(
    source: &LatticeBox,
    sector: &[bool],
    shift: &[i64],
    destinations: &[DestinationScope<'_>],
) -> bool {
    query(Default::default(), source, sector, shift, destinations).unwrap()
}

fn query(
    limits: CompletionGeometryLimits,
    source: &LatticeBox,
    sector: &[bool],
    shift: &[i64],
    destinations: &[DestinationScope<'_>],
) -> Result<bool, ArtifactError> {
    PreparedSuccessorScope::try_new(source.arity(), destinations, limits)?
        .contains(source, sector, shift)
}

#[test]
fn all_four_physical_shift_maps_are_exact() {
    let source = cell([2], [Some(4)]);
    for (active, shift, child_active, lower, upper) in [
        (true, 3, true, 5, 7),
        (true, -6, false, 1, 3),
        (false, 7, true, 2, 4),
        (false, -3, false, 5, 7),
    ] {
        let exact = [cell([lower], [Some(upper)])];
        assert!(contains(
            &source,
            &[active],
            &[shift],
            &[DestinationScope {
                sector: &[child_active],
                boxes: &exact
            }]
        ));
        let missing_endpoint = [cell([lower], [Some(upper - 1)])];
        assert!(!contains(
            &source,
            &[active],
            &[shift],
            &[DestinationScope {
                sector: &[child_active],
                boxes: &missing_endpoint
            }]
        ));
    }
}

#[test]
fn active_to_inactive_crossing_retains_zero_and_the_infinite_positive_tail() {
    let source = cell([0], [None]);
    let inactive = [cell([0], [Some(1)])];
    let active = [cell([0], [None])];
    let domains = [
        DestinationScope {
            sector: &[false],
            boxes: &inactive,
        },
        DestinationScope {
            sector: &[true],
            boxes: &active,
        },
    ];
    assert!(contains(&source, &[true], &[-2], &domains));
    assert!(!contains(&source, &[true], &[-2], &domains[1..]));
    let misses_zero = [cell([1], [Some(1)])];
    assert!(!contains(
        &source,
        &[true],
        &[-2],
        &[
            DestinationScope {
                sector: &[false],
                boxes: &misses_zero
            },
            DestinationScope {
                sector: &[true],
                boxes: &active
            },
        ]
    ));
    let finite_active = [cell([0], [Some(100)])];
    assert!(!contains(
        &source,
        &[true],
        &[-2],
        &[
            DestinationScope {
                sector: &[false],
                boxes: &inactive
            },
            DestinationScope {
                sector: &[true],
                boxes: &finite_active
            },
        ]
    ));
    let maximum_finite = [cell([0], [Some(u64::MAX)])];
    assert!(matches!(
        query(
            Default::default(),
            &source,
            &[true],
            &[-2],
            &[
                DestinationScope {
                    sector: &[false],
                    boxes: &inactive
                },
                DestinationScope {
                    sector: &[true],
                    boxes: &maximum_finite
                },
            ],
        ),
        Err(ArtifactError::ResourceCountOverflow {
            resource: "box-intersection successor coordinate"
        })
    ));
}

#[test]
fn inactive_to_active_crossing_reverses_only_the_finite_piece() {
    let source = cell([0], [None]);
    let active = [cell([0], [Some(1)])];
    let inactive = [cell([0], [None])];
    assert!(contains(
        &source,
        &[false],
        &[2],
        &[
            DestinationScope {
                sector: &[true],
                boxes: &active
            },
            DestinationScope {
                sector: &[false],
                boxes: &inactive
            },
        ]
    ));
    let (sector, image) = translated_image(&cell([0], [Some(1)]), &[false], &[2]).unwrap();
    assert_eq!(sector, [true]);
    assert_eq!(image, active[0]);
    assert!(translated_image(&source, &[false], &[2]).is_err());
}

#[test]
fn repeated_destination_sectors_form_a_union_without_dropping_holes() {
    let source = cell([0], [Some(3)]);
    let left = [cell([0], [Some(1)])];
    let right = [cell([2], [Some(3)])];
    assert!(contains(
        &source,
        &[true],
        &[0],
        &[
            DestinationScope {
                sector: &[true],
                boxes: &left
            },
            DestinationScope {
                sector: &[true],
                boxes: &right
            },
        ]
    ));
    let hole = [cell([3], [Some(3)])];
    assert!(!contains(
        &source,
        &[true],
        &[0],
        &[
            DestinationScope {
                sector: &[true],
                boxes: &left
            },
            DestinationScope {
                sector: &[true],
                boxes: &hole
            },
        ]
    ));
}

#[test]
fn rank_escape_from_arbitrary_dot_ray_is_not_hidden_by_descent() {
    let source = cell([2, 0], [None, Some(0)]);
    let rank_zero = [cell([0, 0], [None, Some(0)])];
    let rank_one = [cell([0, 0], [None, Some(1)])];
    for dots in [3, 4, 100, i64::MAX] {
        for order in [
            OrderingPolicy::RustRedUnshiftedV1,
            OrderingPolicy::SpiredUncutV1,
        ] {
            order
                .prove_strict_descent(&[dots, 0], &[dots - 2, -1])
                .unwrap();
        }
    }
    assert!(!contains(
        &source,
        &[true, false],
        &[-2, -1],
        &[DestinationScope {
            sector: &[true, false],
            boxes: &rank_zero
        }]
    ));
    assert!(contains(
        &source,
        &[true, false],
        &[-2, -1],
        &[DestinationScope {
            sector: &[true, false],
            boxes: &rank_one
        }]
    ));
}

#[test]
fn a_coordinate_permutation_preserves_all_child_sectors_and_images() {
    let source = cell([0, 0], [None, None]);
    let images = [
        ([false, true], cell([0, 0], [Some(1), Some(0)])),
        ([false, false], cell([0, 0], [Some(1), None])),
        ([true, true], cell([0, 0], [None, Some(0)])),
        ([true, false], cell([0, 0], [None, None])),
    ];
    let domains: Vec<_> = images
        .iter()
        .map(|(sector, cell)| DestinationScope {
            sector,
            boxes: std::slice::from_ref(cell),
        })
        .collect();
    assert!(contains(&source, &[true, false], &[-2, 1], &domains));
    let swapped: Vec<_> = images
        .iter()
        .map(|(sector, image)| {
            (
                [sector[1], sector[0]],
                cell(
                    [image.lower()[1], image.lower()[0]],
                    [image.upper()[1], image.upper()[0]],
                ),
            )
        })
        .collect();
    let swapped_domains: Vec<_> = swapped
        .iter()
        .map(|(sector, cell)| DestinationScope {
            sector,
            boxes: std::slice::from_ref(cell),
        })
        .collect();
    assert!(contains(
        &source,
        &[false, true],
        &[1, -2],
        &swapped_domains
    ));
    assert!(!contains(
        &source,
        &[false, true],
        &[1, -2],
        &swapped_domains[..3]
    ));
}

#[test]
fn wide_mathematical_geometry_does_not_claim_machine_arithmetic_safety() {
    let beyond_i64 = (1_u64 << 63) + 1;
    let original = cell([1_u64 << 63], [Some(1_u64 << 63)]);
    let expected = [cell([beyond_i64], [Some(beyond_i64)])];
    assert!(contains(
        &original,
        &[false],
        &[-1],
        &[DestinationScope {
            sector: &[false],
            boxes: &expected
        }]
    ));
    assert!(i64::MIN.checked_add(-1).is_none());
    // Containment above is mathematical only. The existing executable-cell
    // machine-domain admission must separately reject this particular key.
    let maximum = cell([u64::MAX], [Some(u64::MAX)]);
    let full = [cell([0], [None])];
    assert!(contains(
        &maximum,
        &[true],
        &[0],
        &[DestinationScope {
            sector: &[true],
            boxes: &full
        }]
    ));
    for (active, shift) in [(true, 1), (false, -1)] {
        assert!(matches!(
            query(
                Default::default(),
                &maximum,
                &[active],
                &[shift],
                &[DestinationScope {
                    sector: &[active],
                    boxes: &full
                }]
            ),
            Err(ArtifactError::InvalidRuleShape {
                detail: "successor local coordinate exceeds u64"
            })
        ));
    }
    let extreme = cell([0], [Some(0)]);
    let negative = [cell([(1_u64 << 63) - 1], [Some((1_u64 << 63) - 1)])];
    assert!(contains(
        &extreme,
        &[true],
        &[i64::MIN],
        &[DestinationScope {
            sector: &[false],
            boxes: &negative
        }]
    ));
}

#[test]
fn complete_destination_admission_precedes_any_success_or_missing_sector_result() {
    let source = cell([0], [None]);
    let good = [cell([0], [None])];
    let malformed = [cell([0, 0], [None, None])];
    for first_sector in [true, false] {
        let destinations = [
            DestinationScope {
                sector: &[first_sector],
                boxes: &good,
            },
            DestinationScope {
                sector: &[true],
                boxes: &malformed,
            },
        ];
        assert!(query(Default::default(), &source, &[true], &[0], &destinations).is_err());
    }
    assert!(query(Default::default(), &source, &[], &[], &[]).is_err());
    assert!(query(Default::default(), &source, &[true], &[], &[]).is_err());
    assert!(!contains(&source, &[true], &[0], &[]));
}

#[test]
fn exact_and_aggregate_allocation_work_budgets_are_charged_before_calls() {
    let source = cell([0], [None]);
    let boxes = [cell([0], [None])];
    let destinations = [DestinationScope {
        sector: &[true],
        boxes: &boxes,
    }];
    let limits = CompletionGeometryLimits {
        max_requested_boxes: 8,
        max_requested_box_coordinate_cells: 17,
        max_split_operations: 20,
        ..Default::default()
    };
    let mut prepared = PreparedSuccessorScope::try_new(1, &destinations, limits).unwrap();
    assert!(prepared.contains(&source, &[true], &[0]).unwrap());
    assert_eq!(
        (
            prepared.budget.requested_boxes,
            prepared.budget.requested_coordinates,
            prepared.budget.work
        ),
        (5, 10, 11)
    );
    assert!(prepared.contains(&source, &[true], &[0]).unwrap());
    assert_eq!(
        (
            prepared.budget.requested_boxes,
            prepared.budget.requested_coordinates,
            prepared.budget.work
        ),
        (8, 17, 20)
    );
    assert!(prepared.contains(&source, &[true], &[0]).is_err());
    for limits in [
        CompletionGeometryLimits {
            max_requested_boxes: 4,
            ..Default::default()
        },
        CompletionGeometryLimits {
            max_requested_box_coordinate_cells: 9,
            ..Default::default()
        },
        CompletionGeometryLimits {
            max_split_operations: 10,
            ..Default::default()
        },
    ] {
        assert!(query(limits, &source, &[true], &[0], &destinations).is_err());
    }
}

#[test]
fn geometry_failure_consumes_the_unknown_partial_work_allowance() {
    let source = cell([0], [None]);
    let boxes = [cell([0], [None])];
    let destinations = [DestinationScope {
        sector: &[true],
        boxes: &boxes,
    }];
    let limits = CompletionGeometryLimits {
        max_split_operations: 10,
        ..Default::default()
    };
    let mut prepared = PreparedSuccessorScope::try_new(1, &destinations, limits).unwrap();
    assert!(prepared.contains(&source, &[true], &[0]).is_err());
    assert_eq!(prepared.budget.work, limits.max_split_operations);
    assert!(prepared.contains(&source, &[true], &[0]).is_err());
}

#[test]
fn prepared_destinations_are_owned_and_not_recopied_per_rhs() {
    let source = cell([0], [Some(3)]);
    let mut original = vec![cell([0], [Some(1)]), cell([2], [Some(3)])];
    let mut prepared = PreparedSuccessorScope::try_new(
        1,
        &[DestinationScope {
            sector: &[true],
            boxes: &original,
        }],
        Default::default(),
    )
    .unwrap();
    original.clear();
    let pointer = prepared.destinations[0].1.boxes().as_ptr();
    let before = (
        prepared.budget.requested_boxes,
        prepared.budget.requested_coordinates,
    );
    for ordinal in 1..=3 {
        assert!(prepared.contains(&source, &[true], &[0]).unwrap());
        assert_eq!(prepared.destinations[0].1.boxes().as_ptr(), pointer);
        // Only partition/image allocations accrue: no destination box or
        // endpoint copy, despite two boxes in the retained union.
        assert_eq!(prepared.budget.requested_boxes, before.0 + ordinal * 3);
        assert_eq!(
            prepared.budget.requested_coordinates,
            before.1 + ordinal * 7
        );
    }
    assert!(
        !prepared
            .contains(&cell([0], [Some(4)]), &[true], &[0])
            .unwrap()
    );
    assert_eq!(prepared.destinations[0].1.boxes().as_ptr(), pointer);
}

#[test]
fn inconclusive_queries_share_the_uncovered_output_allowance() {
    let source = cell([0], [Some(1)]);
    let mut prepared = PreparedSuccessorScope::try_new(
        1,
        &[DestinationScope {
            sector: &[true],
            boxes: &[],
        }],
        CompletionGeometryLimits {
            max_uncovered_boxes: 1,
            max_uncovered_box_coordinate_cells: 2,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!prepared.contains(&source, &[true], &[0]).unwrap());
    assert_eq!(prepared.budget.uncovered_boxes, 1);
    assert_eq!(prepared.budget.uncovered_coordinates, 2);
    assert!(matches!(
        prepared.contains(&source, &[true], &[0]),
        Err(ArtifactError::ResourceLimit {
            resource: "uncovered lattice boxes",
            requested: 1,
            limit: 0
        })
    ));
}

#[test]
fn prospective_sign_product_and_late_input_caps_fail_without_truncation() {
    let source = cell([0; 64], [None; 64]);
    assert!(matches!(
        query(Default::default(), &source, &[true; 64], &[-1; 64], &[]),
        Err(ArtifactError::ResourceCountOverflow { .. })
    ));
    let limits = CompletionGeometryLimits {
        max_uncovered_boxes: 1,
        ..Default::default()
    };
    assert!(query(limits, &cell([0], [None]), &[true], &[-1], &[]).is_err());
    let boxes = [cell([0], [None]), cell([0], [None])];
    let limits = CompletionGeometryLimits {
        max_requested_boxes: 2,
        ..Default::default()
    };
    assert!(
        query(
            limits,
            &cell([0], [None]),
            &[true],
            &[0],
            &[DestinationScope {
                sector: &[true],
                boxes: &boxes
            }]
        )
        .is_err()
    );
}
