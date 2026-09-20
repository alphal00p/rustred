use super::*;

fn compare_partition<const N: usize>(
    lower: [u64; N],
    upper: [Option<u64>; N],
    sector: [bool; N],
    shift: [i64; N],
    singleton: bool,
) {
    let source = LatticeBox::try_new(lower, upper).unwrap();
    let expected =
        geometry::sign_partition_with_limits(&source, &sector, &shift, Default::default()).unwrap();
    let mut budget = EnvelopeBudget::new(Default::default());
    let actual = budget.partition(&source, &sector, &shift).unwrap();
    assert_eq!(actual.as_slice(), expected);
    assert_eq!(actual.is_borrowed(), singleton);
    if singleton {
        assert!(std::ptr::eq(&actual.as_slice()[0], &source));
    }
}

#[test]
fn borrowed_singletons_and_owned_splits_match_existing_geometry_at_extremes() {
    compare_partition([0], [None], [true], [0], true);
    compare_partition([0], [Some(u64::MAX)], [false], [0], true);
    compare_partition([1], [None], [true], [-1], true);
    compare_partition([0], [Some(0)], [true], [-1], true);
    compare_partition([0], [Some(1)], [true], [-1], false);
    compare_partition([1_u64 << 63], [None], [true], [i64::MIN], true);
    compare_partition([0], [Some((1_u64 << 63) - 1)], [true], [i64::MIN], true);
    compare_partition([0], [None], [true], [i64::MIN], false);
    compare_partition([i64::MAX as u64], [None], [false], [i64::MAX], true);
    compare_partition(
        [0, 1, 0],
        [Some(0), None, Some(u64::MAX)],
        [true, true, false],
        [-1, -1, 0],
        true,
    );
    compare_partition([0, 0, 0], [None; 3], [true, false, true], [-1, 2, 0], false);
    compare_partition([0; 10], [None; 10], [false; 10], [0; 10], true);
}

#[test]
fn singleton_preflight_keeps_shape_arity_and_per_query_limits() {
    let source = LatticeBox::try_new([0; 3], [None; 3]).unwrap();
    for (sector, shift) in [
        (&[true, true][..], &[0, 0][..]),
        (&[true; 3][..], &[0, 0][..]),
    ] {
        assert!(
            EnvelopeBudget::new(Default::default())
                .partition(&source, sector, shift)
                .is_err()
        );
    }
    let mut limits = CompletionGeometryLimits::default();
    limits.max_arity = 2;
    assert!(
        EnvelopeBudget::new(limits)
            .partition(&source, &[true; 3], &[0; 3])
            .is_err()
    );
    for (boxes, coordinates, exceeded) in
        [(0, 6, [true, false, false]), (1, 5, [false, true, false])]
    {
        let limits = CompletionGeometryLimits {
            max_uncovered_boxes: boxes,
            max_uncovered_box_coordinate_cells: coordinates,
            ..Default::default()
        };
        let mut budget = EnvelopeBudget::new(limits);
        assert!(matches!(
            budget.partition(&source, &[true; 3], &[0; 3]),
            Err(SourcePortAuditError::ResourceBudgetExhausted {
                resource: "total-excess sign partition"
            })
        ));
        let snapshot = budget.snapshot(true, false);
        assert_eq!(snapshot.consumed, SourcePortSuccessorCounts::default());
        let attempt = snapshot.failed_attempt.unwrap();
        assert!(attempt.partition_policy);
        assert_eq!(attempt.attempted, [Some(1), Some(6), Some(0)]);
        assert_eq!(attempt.exceeded, exceeded);
    }
}

#[test]
fn singleton_accounting_boundaries_are_cumulative_and_failure_is_not_consumption() {
    let source = LatticeBox::try_new([0; 3], [None; 3]).unwrap();
    let limits = CompletionGeometryLimits {
        max_requested_boxes: 1,
        max_requested_box_coordinate_cells: 18,
        max_split_operations: 21,
        ..Default::default()
    };
    let mut budget = EnvelopeBudget::new(limits);
    budget.partition(&source, &[true; 3], &[0; 3]).unwrap();
    let consumed = SourcePortSuccessorCounts {
        boxes: 1,
        coordinate_cells: 18,
        work: 21,
    };
    assert_eq!(budget.snapshot(false, false).consumed, consumed);
    for request in [2, 3] {
        assert!(budget.partition(&source, &[true; 3], &[0; 3]).is_err());
        let snapshot = budget.snapshot(true, false);
        assert_eq!(snapshot.partition_requests, request);
        assert_eq!(snapshot.consumed, consumed);
        let attempt = snapshot.failed_attempt.unwrap();
        assert!(!attempt.partition_policy);
        assert_eq!(attempt.increment, [Some(1), Some(18), Some(21)]);
        assert_eq!(attempt.attempted, [Some(2), Some(36), Some(42)]);
        assert_eq!(attempt.exceeded, [true; 3]);
        assert_eq!(attempt.overflow, [false; 3]);
    }
    for limits in [
        CompletionGeometryLimits {
            max_requested_boxes: 0,
            ..limits
        },
        CompletionGeometryLimits {
            max_requested_box_coordinate_cells: 17,
            ..limits
        },
        CompletionGeometryLimits {
            max_split_operations: 20,
            ..limits
        },
    ] {
        let mut budget = EnvelopeBudget::new(limits);
        assert!(budget.partition(&source, &[true; 3], &[0; 3]).is_err());
        assert_eq!(
            budget.snapshot(true, false).consumed,
            SourcePortSuccessorCounts::default()
        );
    }
}

#[test]
fn budget_overflow_and_observation_overflow_never_wrap_or_reset_proof_usage() {
    let limits = CompletionGeometryLimits {
        max_requested_boxes: usize::MAX,
        max_requested_box_coordinate_cells: usize::MAX,
        max_split_operations: usize::MAX,
        ..Default::default()
    };
    let mut budget = EnvelopeBudget::new(limits);
    budget.charge(usize::MAX, usize::MAX, usize::MAX).unwrap();
    assert!(budget.charge(1, 1, 1).is_err());
    let snapshot = budget.snapshot(true, false);
    assert_eq!(snapshot.failed_attempt.unwrap().attempted, [None; 3]);
    assert_eq!(snapshot.failed_attempt.unwrap().overflow, [true; 3]);
    assert!(snapshot.arithmetic_overflow);
    assert_eq!(
        snapshot.consumed,
        SourcePortSuccessorCounts {
            boxes: usize::MAX,
            coordinate_cells: usize::MAX,
            work: usize::MAX
        }
    );
    let mut budget = EnvelopeBudget::new(Default::default());
    budget.observation.partition_requests = usize::MAX;
    budget
        .partition(&LatticeBox::try_new([0], [None]).unwrap(), &[true], &[0])
        .unwrap();
    let snapshot = budget.snapshot(false, false);
    assert!(snapshot.telemetry_overflow);
    assert!(!snapshot.arithmetic_overflow);
    assert_eq!(snapshot.partition_requests, usize::MAX);
    assert_eq!(
        snapshot.consumed,
        SourcePortSuccessorCounts {
            boxes: 1,
            coordinate_cells: 6,
            work: 7
        }
    );
    // More than machine-word arity is still a generic geometry request, not
    // loop dispatch. An exponential piece count fails checked arithmetic.
    let n = usize::BITS as usize + 1;
    let source = LatticeBox::try_new(vec![0; n], vec![None; n]).unwrap();
    let mut budget = EnvelopeBudget::new(Default::default());
    assert!(
        budget
            .partition(&source, &vec![true; n], &vec![-1; n])
            .is_err()
    );
    assert!(budget.snapshot(true, false).arithmetic_overflow);
}

#[test]
fn lazy_source_key_is_built_once_and_shortcuts_build_no_keys() {
    let family = crate::foundry::artifact::two_loop::canonical_family(Default::default()).unwrap();
    let entry = EntryScope::try_new(
        &family,
        &Mask::try_new([true; 3]).unwrap(),
        EntryDegreeBound::MaxTotalExcessDegree(3),
    )
    .unwrap();
    let context = CoefficientContext::new(["d", "n0", "n1", "n2"]);
    let application = [
        LatticeBox::try_new([0; 3], [Some(0); 3]).unwrap(),
        LatticeBox::try_new([0, 0, 1], [Some(0), Some(0), Some(1)]).unwrap(),
    ];
    for (shift, zero, expected_keys) in [
        ([-1, 0, 0], false, (1, 2)),
        ([0; 3], false, (0, 0)),
        ([-1, 0, 0], true, (0, 0)),
    ] {
        let mut budget = EnvelopeBudget::new(Default::default());
        let mut obligations = Vec::new();
        visit_successor_degrees(
            &entry,
            OrderingPolicy::SpiredUncutV1,
            &[true, true, false],
            3,
            &application,
            &shift,
            &context.one(),
            None,
            &[],
            |_| zero,
            &[1, 2, 3],
            &mut budget,
            |child, bound| {
                obligations.push((child.to_vec(), bound));
                Ok(())
            },
        )
        .unwrap();
        let snapshot = budget.snapshot(false, true);
        assert_eq!(
            (snapshot.source_key_builds, snapshot.child_key_builds),
            expected_keys
        );
        assert_eq!(snapshot.source_degree_probes, 2);
        assert_eq!(snapshot.skipped_singleton_probes, 2);
        assert_eq!(snapshot.piece_degree_probes, 0);
        if expected_keys.0 == 1 {
            assert_eq!(obligations, vec![(vec![false, true, false], 5); 2]);
        } else {
            assert!(obligations.is_empty());
        }
    }
    let mut budget = EnvelopeBudget::new(Default::default());
    assert!(
        visit_successor_degrees(
            &entry,
            OrderingPolicy::RustRedUnshiftedV1,
            &[true, true, false],
            3,
            &application,
            &[-1, 0, 0],
            &context.one(),
            None,
            &[],
            |_| false,
            &[1, 2, 3],
            &mut budget,
            |_, _| Ok(())
        )
        .is_err()
    );
    assert_eq!(budget.snapshot(true, false).child_key_builds, 0);
}

#[test]
fn reused_keys_match_existing_comparator_for_natural_and_priority_orders() {
    for ordering in [
        OrderingPolicy::SpiredUncutV1,
        sector_ordering([true; 3], Some([2, 0, 1])).unwrap(),
    ] {
        for source_bits in 0..8 {
            let source: [i64; 3] =
                std::array::from_fn(|axis| i64::from(source_bits & (1 << axis) != 0));
            let key = ordering.complexity_key(&source).unwrap();
            for child_bits in 0..8 {
                let child: [i64; 3] =
                    std::array::from_fn(|axis| i64::from(child_bits & (1 << axis) != 0));
                assert_eq!(
                    ordering.complexity_key(&child).unwrap().cmp(&key),
                    ordering.compare(&child, &source).unwrap()
                );
            }
        }
    }
}

#[test]
fn retained_observer_distinguishes_local_report_from_completed_successors() {
    let (audit, solution) = solved_tadpole();
    let mut snapshots = Vec::new();
    let mut local_reports = 0;
    audit
        .audit_complete_through_total_excess_with_observer(
            &tadpole(),
            [([true], None, solution)],
            3,
            |event| match event {
                SourcePortInstallEvent::CheckedSector { .. } => local_reports += 1,
                SourcePortInstallEvent::SuccessorGeometry {
                    sector, snapshot, ..
                } => {
                    assert_eq!(sector, Some(&[true][..]));
                    assert_eq!(local_reports, 1);
                    snapshots.push(*snapshot);
                }
                _ => {}
            },
        )
        .unwrap();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].completed_sectors, 1);
    assert_eq!(snapshots[0].stage, SourcePortSuccessorStage::Retained);
    assert!(!snapshots[0].failed);
    assert!(snapshots[0].traversal_complete);
    let (audit, solution) = solved_tadpole();
    let mut limits = super::super::super::SourcePortLimits::default();
    limits.cover_replay.max_requested_boxes = 0;
    let mut failed = None;
    assert!(
        audit
            .with_limits(limits)
            .audit_complete_through_total_excess_with_observer(
                &tadpole(),
                [([true], None, solution)],
                3,
                |event| {
                    if let SourcePortInstallEvent::SuccessorGeometry {
                        sector, snapshot, ..
                    } = event
                    {
                        assert!(sector.is_none());
                        failed = Some(*snapshot);
                    }
                }
            )
            .is_err()
    );
    let failed = failed.unwrap();
    assert!(failed.failed);
    assert!(!failed.traversal_complete);
    assert_eq!(failed.completed_sectors, 0);
    assert_eq!(failed.consumed, SourcePortSuccessorCounts::default());
    assert_eq!(
        failed.failed_attempt.unwrap().exceeded,
        [true, false, false]
    );
}

#[test]
fn successor_obligations_match_old_owned_geometry_and_comparison() {
    let family = crate::foundry::artifact::two_loop::canonical_family(Default::default()).unwrap();
    let entry = EntryScope::try_new(
        &family,
        &Mask::try_new([true; 3]).unwrap(),
        EntryDegreeBound::MaxTotalExcessDegree(3),
    )
    .unwrap();
    let context = CoefficientContext::new(["d", "n0", "n1", "n2"]);
    let applications = [
        LatticeBox::try_new([0; 3], [None; 3]).unwrap(),
        LatticeBox::try_new([1, 0, 0], [Some(1), Some(0), Some(0)]).unwrap(),
    ];
    for ordering in [
        OrderingPolicy::SpiredUncutV1,
        sector_ordering([true; 3], Some([2, 0, 1])).unwrap(),
    ] {
        for bits in 0..8 {
            let sector: [bool; 3] = std::array::from_fn(|axis| bits & (1 << axis) != 0);
            for shift in [[0; 3], [-1, 0, 0], [0, -1, 1], [1, -1, -1]] {
                // Test-only old-owned reference for the exact constant-one
                // coefficient case. It grants no CheckedRule or owner seal.
                let reference = (|| {
                    let mut obligations = Vec::new();
                    for source in &applications {
                        if !EntryDegreeBound::MaxTotalExcessDegree(3)
                            .intersects_local_box(&sector, source)
                            .unwrap()
                        {
                            continue;
                        }
                        for piece in geometry::sign_partition_with_limits(
                            source,
                            &sector,
                            &shift,
                            Default::default(),
                        )
                        .unwrap()
                        {
                            if !EntryDegreeBound::MaxTotalExcessDegree(3)
                                .intersects_local_box(&sector, &piece)
                                .unwrap()
                            {
                                continue;
                            }
                            let child: [bool; 3] = std::array::from_fn(|axis| {
                                let local = i128::from(piece.lower()[axis]);
                                (if sector[axis] { local + 1 } else { -local })
                                    + i128::from(shift[axis])
                                    > 0
                            });
                            if child == sector {
                                continue;
                            }
                            if ordering
                                .compare(&child.map(i64::from), &sector.map(i64::from))
                                .unwrap()
                                != Ordering::Less
                            {
                                return Err(error(format!(
                                    "unresolved nonlower or out-of-root successor sector {sector:?} -> {child:?}, shift={shift:?}"
                                )));
                            }
                            obligations.push((
                                child.to_vec(),
                                successor_degree(3, &sector, &child, &shift).unwrap(),
                            ));
                        }
                    }
                    Ok(obligations)
                })();
                let mut actual = Vec::new();
                let result = visit_successor_degrees(
                    &entry,
                    ordering,
                    &sector,
                    3,
                    &applications,
                    &shift,
                    &context.one(),
                    None,
                    &[],
                    |_| false,
                    &[1, 2, 3],
                    &mut EnvelopeBudget::new(Default::default()),
                    |child, bound| {
                        actual.push((child.to_vec(), bound));
                        Ok(())
                    },
                );
                assert_eq!(
                    result.map(|()| actual).map_err(|e| e.to_string()),
                    reference.map_err(|e| e.to_string())
                );
            }
        }
    }
}
