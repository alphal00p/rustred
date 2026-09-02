use super::super::super::CompletionGeometryLimits;
use super::super::*;
use super::support::*;

#[test]
fn aggregate_mask_queue_and_blind_priority_caps_reject_before_retention() {
    let defaults = InvolutiveLimits::default();
    let context = context(2);
    let ordering = active_ordering(2, defaults);
    let build_two_leaders = |limits| {
        JanetBasisEpoch::try_initial(
            [
                monomial_consequence(0, &[2, 0], &ordering, &context, defaults),
                monomial_consequence(1, &[0, 3], &ordering, &context, defaults),
            ],
            &ordering,
            &context,
            limits,
            CompletionGeometryLimits::default(),
        )
    };

    let basis_rows = InvolutiveLimits {
        max_basis_rows: 1,
        ..defaults
    };
    assert_eq!(
        build_two_leaders(basis_rows),
        Err(InvolutiveError::ResourceLimit {
            resource: "Janet basis rows",
            requested: 2,
            limit: 1,
        })
    );
    let basis_cells = InvolutiveLimits {
        max_basis_coordinate_cells: 3,
        ..defaults
    };
    assert_eq!(
        build_two_leaders(basis_cells),
        Err(InvolutiveError::ResourceLimit {
            resource: "Janet basis coordinate cells",
            requested: 4,
            limit: 3,
        })
    );
    let mask_prefix = InvolutiveLimits {
        max_mask_prefix_comparisons: 0,
        ..defaults
    };
    assert_eq!(
        build_two_leaders(mask_prefix),
        Err(InvolutiveError::ResourceLimit {
            resource: "Janet mask prefix comparisons",
            requested: 1,
            limit: 0,
        })
    );
    let mask_sort = InvolutiveLimits {
        max_mask_sort_coordinate_comparisons: 3,
        ..defaults
    };
    assert_eq!(
        build_two_leaders(mask_sort),
        Err(InvolutiveError::ResourceLimit {
            resource: "Janet mask sort coordinate comparisons",
            requested: 4,
            limit: 3,
        })
    );
    let mask_bytes_required =
        4 * std::mem::size_of::<bool>() + 2 * std::mem::size_of::<Vec<bool>>();
    let mask_bytes = InvolutiveLimits {
        max_mask_retained_bytes: mask_bytes_required - 1,
        ..defaults
    };
    assert_eq!(
        build_two_leaders(mask_bytes),
        Err(InvolutiveError::ResourceLimit {
            resource: "Janet mask retained bytes",
            requested: mask_bytes_required,
            limit: mask_bytes_required - 1,
        })
    );
    let queue_cells = InvolutiveLimits {
        max_prolongation_coordinate_cells: 3,
        ..defaults
    };
    assert_eq!(
        build_two_leaders(queue_cells),
        Err(InvolutiveError::ResourceLimit {
            resource: "Janet prolongation coordinate cells",
            requested: 4,
            limit: 3,
        })
    );
    let queue_bytes_required = 2 * (std::mem::size_of::<u64>() + std::mem::size_of::<i128>())
        + std::mem::size_of::<JanetProlongation>();
    let queue_bytes = InvolutiveLimits {
        max_prolongation_retained_bytes: queue_bytes_required - 1,
        ..defaults
    };
    assert_eq!(
        build_two_leaders(queue_bytes),
        Err(InvolutiveError::ResourceLimit {
            resource: "Janet prolongation retained bytes",
            requested: queue_bytes_required,
            limit: queue_bytes_required - 1,
        })
    );

    let residual = epoch(&[&[2, 2]], &context, &ordering, defaults);
    let blind_cells = InvolutiveLimits {
        max_blind_coordinate_cells: 7,
        ..defaults
    };
    assert_eq!(
        BlindDomainSchedule::try_from_partition(
            residual.uncovered_partition(),
            &ordering,
            blind_cells,
        ),
        Err(InvolutiveError::ResourceLimit {
            resource: "blind-domain endpoint cells",
            requested: 8,
            limit: 7,
        })
    );
    let schedule = BlindDomainSchedule::try_from_partition(
        residual.uncovered_partition(),
        &ordering,
        defaults,
    )
    .unwrap();
    let candidates = epoch(&[&[2, 0], &[1, 2], &[0, 3]], &context, &ordering, defaults);
    assert_eq!(candidates.prolongations().len(), 2);
    let candidate_cap = InvolutiveLimits {
        max_priority_candidates: 1,
        ..defaults
    };
    assert_eq!(
        schedule.try_rank_prolongation_ordinals(&candidates, &ordering, candidate_cap,),
        Err(InvolutiveError::ResourceLimit {
            resource: "blind-domain priority candidates",
            requested: 2,
            limit: 1,
        })
    );
    let intersection_cap = InvolutiveLimits {
        max_blind_priority_intersection_cells: 7,
        ..defaults
    };
    assert_eq!(
        schedule.try_rank_prolongation_ordinals(&candidates, &ordering, intersection_cap,),
        Err(InvolutiveError::ResourceLimit {
            resource: "blind-domain priority intersection cells",
            requested: 8,
            limit: 7,
        })
    );
    let priority_sort_cap = InvolutiveLimits {
        max_blind_priority_sort_coordinate_comparisons: 3,
        ..defaults
    };
    assert_eq!(
        schedule.try_rank_prolongation_ordinals(&candidates, &ordering, priority_sort_cap,),
        Err(InvolutiveError::ResourceLimit {
            resource: "blind-domain priority sort coordinate comparisons",
            requested: 4,
            limit: 3,
        })
    );
    let retained_probe = InvolutiveLimits {
        max_blind_priority_retained_bytes: 0,
        ..defaults
    };
    let retained_bytes = match schedule
        .try_rank_prolongation_ordinals(&candidates, &ordering, retained_probe)
        .unwrap_err()
    {
        InvolutiveError::ResourceLimit {
            resource: "blind-domain priority retained bytes",
            requested,
            limit: 0,
        } => requested,
        error => panic!("unexpected retained-byte preflight error: {error}"),
    };
    let retained_cap = InvolutiveLimits {
        max_blind_priority_retained_bytes: retained_bytes - 1,
        ..defaults
    };
    assert_eq!(
        schedule.try_rank_prolongation_ordinals(&candidates, &ordering, retained_cap,),
        Err(InvolutiveError::ResourceLimit {
            resource: "blind-domain priority retained bytes",
            requested: retained_bytes,
            limit: retained_bytes - 1,
        })
    );
}

#[test]
fn every_first_slice_limit_rejects_one_unit_below_required_work() {
    let defaults = InvolutiveLimits::default();
    let degree_limited = InvolutiveLimits {
        max_total_shift_degree: 2,
        ..defaults
    };
    assert_eq!(
        ForwardShift::try_new([1, 2], degree_limited),
        Err(InvolutiveError::ResourceLimit {
            resource: "forward-shift total degree",
            requested: 3,
            limit: 2,
        })
    );
    let coordinate_limited = InvolutiveLimits {
        max_shift_coordinate: 2,
        ..defaults
    };
    assert_eq!(
        ForwardShift::try_new([3], coordinate_limited),
        Err(InvolutiveError::ShiftCoordinateLimit {
            position: 0,
            requested: 3,
            limit: 2,
        })
    );

    let context = context(2);
    let ordering = active_ordering(2, defaults);
    let row_limited = InvolutiveLimits {
        max_row_terms: 1,
        ..defaults
    };
    assert_eq!(
        OreRow::try_new(
            &ordering,
            [
                (shift(&[1, 0], defaults), context.one()),
                (shift(&[0, 1], defaults), context.one()),
            ],
            &context,
            row_limited,
        ),
        Err(InvolutiveError::ResourceLimit {
            resource: "Ore row terms",
            requested: 2,
            limit: 1,
        })
    );

    let queue_limited = InvolutiveLimits {
        max_prolongations: 0,
        ..defaults
    };
    assert_eq!(
        JanetBasisEpoch::try_initial(
            [
                monomial_consequence(0, &[2, 0], &ordering, &context, defaults),
                monomial_consequence(1, &[0, 3], &ordering, &context, defaults),
            ],
            &ordering,
            &context,
            queue_limited,
            CompletionGeometryLimits::default(),
        ),
        Err(InvolutiveError::ResourceLimit {
            resource: "Janet prolongations",
            requested: 1,
            limit: 0,
        })
    );

    let epoch_limited = InvolutiveLimits {
        max_epoch: 0,
        ..defaults
    };
    let initial = epoch(&[&[1, 0]], &context, &ordering, epoch_limited);
    assert_eq!(
        initial.try_successor(
            std::iter::empty::<OreConsequence>(),
            &ordering,
            &context,
            epoch_limited,
            CompletionGeometryLimits::default(),
        ),
        Err(InvolutiveError::EpochLimit {
            requested: 1,
            limit: 0,
        })
    );

    let residual = epoch(&[&[2, 2]], &context, &ordering, defaults);
    let blind_limited = InvolutiveLimits {
        max_blind_boxes_scanned: 1,
        ..defaults
    };
    assert_eq!(
        BlindDomainSchedule::try_from_partition(
            residual.uncovered_partition(),
            &ordering,
            blind_limited,
        ),
        Err(InvolutiveError::ResourceLimit {
            resource: "blind-domain boxes scanned",
            requested: 2,
            limit: 1,
        })
    );
}

#[test]
fn coefficient_payload_caps_are_exact_per_consequence_and_across_a_basis() {
    let defaults = InvolutiveLimits::default();
    let context = context(2);
    let ordering = active_ordering(2, defaults);
    let baseline = monomial_consequence(0, &[1, 0], &ordering, &context, defaults);
    let one = baseline.coefficient_census();
    assert!(one.terms() > 0);
    assert!(one.exponent_cells() > 0);
    assert!(one.retained_bytes() > 0);

    let build_consequence = |limits| {
        OreConsequence::try_from_source(
            0,
            OreRow::try_new(
                &ordering,
                [(shift(&[1, 0], defaults), context.one())],
                &context,
                defaults,
            )
            .unwrap(),
            &ordering,
            &context,
            limits,
        )
    };
    for (limits, resource, requested, limit) in [
        (
            InvolutiveLimits {
                max_consequence_coefficient_terms: one.terms() - 1,
                ..defaults
            },
            "Ore consequence coefficient terms",
            one.terms(),
            one.terms() - 1,
        ),
        (
            InvolutiveLimits {
                max_consequence_coefficient_exponent_cells: one.exponent_cells() - 1,
                ..defaults
            },
            "Ore consequence coefficient exponent cells",
            one.exponent_cells(),
            one.exponent_cells() - 1,
        ),
        (
            InvolutiveLimits {
                max_consequence_coefficient_retained_bytes: one.retained_bytes() - 1,
                ..defaults
            },
            "Ore consequence coefficient retained bytes",
            one.retained_bytes(),
            one.retained_bytes() - 1,
        ),
    ] {
        assert_eq!(
            build_consequence(limits),
            Err(InvolutiveError::ResourceLimit {
                resource,
                requested,
                limit,
            })
        );
    }

    let expected_terms = one.terms() * 2;
    let expected_cells = one.exponent_cells() * 2;
    let expected_bytes = one.retained_bytes() * 2;
    let build_basis = |limits| {
        JanetBasisEpoch::try_initial(
            [
                monomial_consequence(0, &[1, 0], &ordering, &context, defaults),
                monomial_consequence(1, &[0, 1], &ordering, &context, defaults),
            ],
            &ordering,
            &context,
            limits,
            CompletionGeometryLimits::default(),
        )
    };
    for (limits, resource, requested, limit) in [
        (
            InvolutiveLimits {
                max_basis_coefficient_terms: expected_terms - 1,
                ..defaults
            },
            "Janet basis coefficient terms",
            expected_terms,
            expected_terms - 1,
        ),
        (
            InvolutiveLimits {
                max_basis_coefficient_exponent_cells: expected_cells - 1,
                ..defaults
            },
            "Janet basis coefficient exponent cells",
            expected_cells,
            expected_cells - 1,
        ),
        (
            InvolutiveLimits {
                max_basis_coefficient_retained_bytes: expected_bytes - 1,
                ..defaults
            },
            "Janet basis coefficient retained bytes",
            expected_bytes,
            expected_bytes - 1,
        ),
    ] {
        assert_eq!(
            build_basis(limits),
            Err(InvolutiveError::ResourceLimit {
                resource,
                requested,
                limit,
            })
        );
    }
}

#[test]
fn monic_basis_admission_bounds_inflated_aggregate_payload_and_exact_work() {
    let defaults = InvolutiveLimits::default();
    let context = context(2);
    let ordering = active_ordering(2, defaults);
    let n0_plus_one = context
        .add(&context.index(0).unwrap(), &context.one())
        .unwrap();
    let n1_plus_one = context
        .add(&context.index(1).unwrap(), &context.one())
        .unwrap();
    let lower = context
        .add(
            &context
                .mul(&context.index(0).unwrap(), &context.index(0).unwrap())
                .unwrap(),
            &n0_plus_one,
        )
        .unwrap();
    let make = |source_ordinal, powers: &[u64], leading| {
        OreConsequence::try_from_source(
            source_ordinal,
            OreRow::try_new(
                &ordering,
                [
                    (shift(&[0, 0], defaults), lower.clone()),
                    (shift(powers, defaults), leading),
                ],
                &context,
                defaults,
            )
            .unwrap(),
            &ordering,
            &context,
            defaults,
        )
        .unwrap()
    };
    let build = |limits| {
        JanetBasisEpoch::try_initial(
            [
                make(0, &[1, 0], n0_plus_one.clone()),
                make(1, &[0, 1], n1_plus_one.clone()),
            ],
            &ordering,
            &context,
            limits,
            CompletionGeometryLimits::default(),
        )
    };
    let baseline = build(defaults).unwrap();
    let normalized_terms = baseline
        .elements()
        .iter()
        .map(|element| element.consequence().coefficient_census().terms())
        .sum::<usize>();
    assert!(normalized_terms > 0);

    let one_below = InvolutiveLimits {
        max_basis_coefficient_terms: normalized_terms - 1,
        ..defaults
    };
    assert_eq!(
        build(one_below),
        Err(InvolutiveError::ResourceLimit {
            resource: "Janet basis coefficient terms",
            requested: normalized_terms,
            limit: normalized_terms - 1,
        })
    );

    // Each row charges one exact inversion and two operations for each of its
    // two row terms plus its one provenance term: seven operations per row.
    let exact_work_one_below = InvolutiveLimits {
        max_exact_coefficient_operations: 13,
        ..defaults
    };
    assert_eq!(
        build(exact_work_one_below),
        Err(InvolutiveError::ResourceLimit {
            resource: "Janet exact coefficient operations",
            requested: 14,
            limit: 13,
        })
    );
}
