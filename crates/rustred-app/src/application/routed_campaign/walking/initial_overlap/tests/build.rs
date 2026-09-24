use super::*;

fn route(serial: usize) -> Arc<Domain<3>> {
    let mut d = domain(None, None, None);
    d.phase = Phase::Route;
    d.upper[0] = Some(serial as u64 + 3);
    Arc::new(d)
}

fn assert_empty<const N: usize>(index: &InitialOverlapIndex<N>) {
    assert!(index.initial.is_empty());
    assert!(index.anchors.is_empty());
    assert_eq!(index.build_report().retained_membership, 0);
    assert_eq!(index.build_report().usable_anchors, 0);
}

#[test]
fn initial_overlap_many_routes_preserve_original_apply_ids_and_first_anchor_order() {
    let first_id = 7;
    let second_id = MAX_INITIAL_DOMAINS + 9;
    let first = Arc::new(domain(Some(2), None, Some(6)));
    let second = Arc::new(domain(Some(1), None, Some(6)));
    let initial: Vec<_> = (0..MAX_INITIAL_DOMAINS + 20)
        .map(|id| match id {
            n if n == first_id => first.clone(),
            n if n == second_id => second.clone(),
            n => route(n),
        })
        .collect();
    let index = InitialOverlapIndex::from_initial(&initial, &stop());
    let report = index.build_report();
    assert_eq!(report.status, InitialOverlapBuildStatus::Active);
    assert_eq!(report.total_initial, initial.len());
    assert_eq!(report.examined_initial, initial.len());
    assert!(report.eligibility_complete);
    assert_eq!(report.eligible_apply, 2);
    assert_eq!(report.retained_membership, 2);
    assert_eq!(report.usable_anchors, 2);
    assert_eq!(
        report.requested_logical_entry_bytes,
        Some(2 * report.logical_bytes_per_eligible_entry)
    );
    assert!(index.initial.contains(&first) && index.initial.contains(&second));
    assert!(!index.initial.contains(&initial[0]));
    let anchors = &index.anchors[&first.owner];
    assert_eq!(
        anchors.iter().map(|a| a.id).collect::<Vec<_>>(),
        [first_id, second_id]
    );
    let plan = index
        .plan(&domain(Some(-1), None, Some(6)), &stop())
        .unwrap();
    assert_eq!(plan.scope.anchor_id, first_id);
    assert_eq!(plan.scope.cut, 2);
    assert!(index.plan(&first, &stop()).is_none());
    assert!(index.plan(&second, &stop()).is_none());
    assert!(index.plan(&initial[0], &stop()).is_none());
}

#[test]
fn initial_overlap_empty_all_route_and_not_requested_are_distinct_nonfailures() {
    let not_requested = InitialOverlapIndex::<3>::empty();
    assert_empty(&not_requested);
    assert_eq!(
        not_requested.build_report().status.as_str(),
        "not_requested"
    );
    assert!(!not_requested.build_report().eligibility_complete);

    let empty = InitialOverlapIndex::<3>::from_initial(&[], &stop());
    assert_empty(&empty);
    assert_eq!(
        empty.build_report().status,
        InitialOverlapBuildStatus::EmptyInitial
    );
    assert!(empty.build_report().eligibility_complete);
    assert_eq!(empty.build_report().requested_logical_entry_bytes, Some(0));

    let initial: Vec<_> = (0..MAX_INITIAL_DOMAINS + 1).map(route).collect();
    let all_route = InitialOverlapIndex::build(
        &initial,
        0,
        0,
        || false,
        || panic!("Route-only input must not reserve optional index storage"),
    );
    assert_empty(&all_route);
    let report = all_route.build_report();
    assert_eq!(report.status, InitialOverlapBuildStatus::NoEligibleApply);
    assert_eq!(report.total_initial, initial.len());
    assert_eq!(report.examined_initial, initial.len());
    assert_eq!(report.eligible_apply, 0);
    assert!(report.eligibility_complete);
    assert_eq!(report.requested_logical_entry_bytes, Some(0));
}

#[test]
fn initial_overlap_apply_count_and_actual_byte_boundaries_remain_enforced() {
    let first = Arc::new(domain(Some(1), None, Some(6)));
    let second = Arc::new(domain(Some(2), None, Some(6)));
    let initial = [route(0), first, route(1), second];
    let entry_bytes = size_of::<Arc<Domain<3>>>()
        + size_of::<Anchor<3>>()
        + size_of::<([bool; 3], Vec<Anchor<3>>)>();
    let exact = InitialOverlapIndex::with_limits(&initial, &stop(), 2, 2 * entry_bytes);
    assert_eq!(
        exact.build_report().status,
        InitialOverlapBuildStatus::Active
    );
    assert_eq!(
        exact.build_report().logical_bytes_per_eligible_entry,
        entry_bytes
    );
    for (count, bytes, expected) in [
        (1, 2 * entry_bytes, InitialOverlapBuildStatus::CountLimit),
        (2, 2 * entry_bytes - 1, InitialOverlapBuildStatus::ByteLimit),
    ] {
        let refused = InitialOverlapIndex::with_limits(&initial, &stop(), count, bytes);
        assert_empty(&refused);
        let report = refused.build_report();
        assert_eq!(report.status, expected);
        assert_eq!(report.eligible_apply, 2);
        assert!(report.eligibility_complete);
        assert_eq!(report.requested_logical_entry_bytes, Some(2 * entry_bytes));
        assert!(
            refused
                .plan(&domain(Some(-1), None, Some(6)), &stop())
                .is_none()
        );
    }

    let many_apply: Vec<_> = (0..=MAX_INITIAL_DOMAINS)
        .map(|serial| {
            let mut d = domain(Some(1), None, None);
            d.upper[0] = Some(serial as u64 + 2);
            Arc::new(d)
        })
        .collect();
    let refused = InitialOverlapIndex::from_initial(&many_apply, &stop());
    assert_empty(&refused);
    assert_eq!(
        refused.build_report().status,
        InitialOverlapBuildStatus::CountLimit
    );
    assert_eq!(
        refused.build_report().eligible_apply,
        MAX_INITIAL_DOMAINS + 1
    );
}

#[test]
fn initial_overlap_unusable_apply_entries_keep_membership_and_global_ids() {
    let mut invalid = domain(None, None, Some(6));
    invalid.lower[0] = 3;
    let empty = domain(Some(7), None, Some(6));
    let mut no_cut = domain(None, None, Some(4));
    no_cut.rank = None;
    no_cut.upper[2] = None;
    let mut minimum_cut = domain(None, None, None);
    minimum_cut.rank = None;
    minimum_cut.upper = vec![Some(0), Some(0), Some((1_u64 << 63) + 2)];
    let mut oversized_cut = domain(None, None, None);
    oversized_cut.lower[0] = i64::MAX as u64;
    oversized_cut.upper[0] = None;
    oversized_cut.upper[2] = Some(0);
    oversized_cut.rank = Some(0);
    let unusable: Vec<_> = [invalid, empty, no_cut, minimum_cut, oversized_cut]
        .into_iter()
        .map(Arc::new)
        .collect();
    let index = InitialOverlapIndex::from_initial(&unusable, &stop());
    let report = index.build_report();
    assert_eq!(report.status, InitialOverlapBuildStatus::NoUsableAnchors);
    assert_eq!(report.eligible_apply, unusable.len());
    assert_eq!(report.retained_membership, unusable.len());
    assert_eq!(report.usable_anchors, 0);
    for initial in &unusable {
        assert!(index.initial.contains(initial));
        assert!(index.plan(initial, &stop()).is_none());
    }

    let mut mixed = vec![route(0)];
    mixed.extend(unusable.iter().cloned());
    let valid_id = mixed.len();
    let valid = Arc::new(domain(Some(1), None, Some(6)));
    mixed.push(valid.clone());
    let index = InitialOverlapIndex::from_initial(&mixed, &stop());
    assert_eq!(
        index.build_report().status,
        InitialOverlapBuildStatus::Active
    );
    assert_eq!(index.build_report().retained_membership, unusable.len() + 1);
    assert_eq!(index.build_report().usable_anchors, 1);
    for initial in &unusable {
        assert!(index.initial.contains(initial));
        assert!(index.plan(initial, &stop()).is_none());
    }
    // The no-cut initial Apply query WOULD be partially pruned by this other
    // anchor if it were incorrectly omitted from the initial-membership set.
    let anchor_only = InitialOverlapIndex::from_initial(&[valid], &stop());
    assert!(anchor_only.plan(&unusable[2], &stop()).is_some());
    assert_eq!(
        index
            .plan(&domain(Some(-1), None, Some(6)), &stop())
            .unwrap()
            .scope
            .anchor_id,
        valid_id
    );
}

#[test]
fn initial_overlap_cancellation_reports_prefix_counts_and_discards_partial_builds() {
    let initial = [
        Arc::new(domain(Some(1), None, Some(6))),
        route(0),
        Arc::new(domain(Some(2), None, Some(6))),
    ];
    // Three count checks, one post-count check, three construction checks and
    // one final check. Exercise cancellation before/after partial storage too.
    for cancel_at in 1..=8 {
        let mut checks = 0;
        let index = InitialOverlapIndex::build(
            &initial,
            MAX_INITIAL_DOMAINS,
            MAX_ENTRY_BYTES,
            || {
                checks += 1;
                checks == cancel_at
            },
            || true,
        );
        assert_empty(&index);
        let report = index.build_report();
        assert_eq!(report.status, InitialOverlapBuildStatus::Cancelled);
        assert_eq!(report.total_initial, 3);
        assert_eq!(report.eligibility_complete, cancel_at > 3);
        if cancel_at <= 3 {
            assert_eq!(report.examined_initial, cancel_at - 1);
            assert_eq!(report.eligible_apply, usize::from(cancel_at > 1));
            assert_eq!(report.requested_logical_entry_bytes, None);
        } else {
            assert_eq!(report.examined_initial, 3);
            assert_eq!(report.eligible_apply, 2);
            assert_eq!(
                report.requested_logical_entry_bytes,
                Some(2 * report.logical_bytes_per_eligible_entry)
            );
        }
    }
    let cancelled = InitialOverlapIndex::from_initial(&initial, &AtomicBool::new(true));
    assert_empty(&cancelled);
    assert!(!cancelled.build_report().eligibility_complete);
    assert_eq!(cancelled.build_report().examined_initial, 0);

    let all_route = [route(0), route(1), route(2)];
    let mut checks = 0;
    let cancelled = InitialOverlapIndex::build(
        &all_route,
        MAX_INITIAL_DOMAINS,
        MAX_ENTRY_BYTES,
        || {
            checks += 1;
            checks == 2
        },
        || panic!("cancellation during Route counting precedes reservations"),
    );
    assert_empty(&cancelled);
    assert_eq!(cancelled.build_report().examined_initial, 1);
    assert_eq!(cancelled.build_report().eligible_apply, 0);
    assert!(!cancelled.build_report().eligibility_complete);
}

#[test]
fn initial_overlap_reserve_failure_discards_all_partial_membership_and_anchors() {
    let initial = [
        Arc::new(domain(Some(1), None, Some(6))),
        route(0),
        Arc::new(domain(Some(2), None, Some(6))),
    ];
    // Membership set, owner map, first anchor, second anchor reservations.
    for deny_at in 1..=4 {
        let mut reservations = 0;
        let index = InitialOverlapIndex::build(
            &initial,
            MAX_INITIAL_DOMAINS,
            MAX_ENTRY_BYTES,
            || false,
            || {
                reservations += 1;
                reservations != deny_at
            },
        );
        assert_empty(&index);
        let report = index.build_report();
        assert_eq!(report.status, InitialOverlapBuildStatus::AllocationFailure);
        assert!(report.eligibility_complete);
        assert_eq!(report.eligible_apply, 2);
        assert_eq!(
            report.requested_logical_entry_bytes,
            Some(2 * report.logical_bytes_per_eligible_entry)
        );
        assert!(
            index
                .plan(&domain(Some(-1), None, Some(6)), &stop())
                .is_none()
        );
    }
}

#[test]
fn initial_overlap_generic_large_route_mix_charges_actual_apply_entry_sizes_only() {
    const N: usize = 16;
    const APPLY_COUNT: usize = 512;
    let make = |phase, serial: usize| {
        let mut upper = vec![Some(1); N];
        upper[0] = Some(serial as u64 + 2);
        Arc::new(Domain::<N> {
            phase,
            owner: std::array::from_fn(|axis| axis % 2 == 0),
            lower: vec![0; N],
            upper,
            rank: Some((N / 2) as u32),
            powers: DomainPowerBounds::default(),
        })
    };
    let mut initial = Vec::new();
    for serial in 0..MAX_INITIAL_DOMAINS + 17 {
        initial.push(make(Phase::Route, serial));
        if serial < APPLY_COUNT {
            initial.push(make(Phase::Apply, serial));
        }
    }
    let entry_bytes = size_of::<Arc<Domain<N>>>()
        + size_of::<Anchor<N>>()
        + size_of::<([bool; N], Vec<Anchor<N>>)>();
    let charge = APPLY_COUNT.checked_mul(entry_bytes).unwrap();
    assert!(charge <= MAX_ENTRY_BYTES);
    let index = InitialOverlapIndex::<N>::from_initial(&initial, &stop());
    let report = index.build_report();
    assert_eq!(report.status, InitialOverlapBuildStatus::Active);
    assert_eq!(report.logical_bytes_per_eligible_entry, entry_bytes);
    assert_eq!(report.requested_logical_entry_bytes, Some(charge));
    assert_eq!(report.eligible_apply, APPLY_COUNT);
    assert_eq!(report.retained_membership, APPLY_COUNT);
    assert_eq!(report.usable_anchors, APPLY_COUNT);
    let owner: [bool; N] = std::array::from_fn(|axis| axis % 2 == 0);
    assert_eq!(index.anchors[&owner][0].id, 1);
    assert_eq!(
        index.anchors[&owner][APPLY_COUNT - 1].id,
        2 * APPLY_COUNT - 1
    );
}
