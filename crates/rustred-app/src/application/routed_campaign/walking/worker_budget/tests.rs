use super::*;

const POLICIES: [OwnerDomainWalkPublicationPolicy; 3] = [
    OwnerDomainWalkPublicationPolicy::Ordered,
    OwnerDomainWalkPublicationPolicy::OwnerBatched,
    OwnerDomainWalkPublicationPolicy::Ready,
];

#[test]
fn automatic_worker_partitions_preserve_both_historical_defaults() {
    for workers in 1..=256 {
        for policy in POLICIES {
            for cap in [None, Some(50)] {
                let budget = WorkerBudget::new(workers, None, cap, policy);
                let threshold = if policy != OwnerDomainWalkPublicationPolicy::OwnerBatched {
                    5
                } else {
                    4
                };
                let helpers = if workers >= threshold && cap.is_none() {
                    let half = (workers - 1) / 2;
                    if policy == OwnerDomainWalkPublicationPolicy::Ready {
                        half.min(READY_HELPER_CAP)
                    } else {
                        half
                    }
                } else {
                    0
                };
                let coordinator = usize::from(workers > 1);
                assert_eq!(budget.helpers, helpers);
                assert_eq!(budget.coordinator, coordinator);
                assert_eq!(budget.inspection, workers - helpers - coordinator);
                assert_eq!(
                    budget.inspection + budget.helpers + budget.coordinator,
                    workers
                );
            }
        }
    }
}

#[test]
fn every_explicit_worker_partition_reserves_exactly_the_configured_total() {
    for workers in 1..=256 {
        let available = if workers == 1 { 1 } else { workers - 1 };
        for inspection in 1..=available {
            for policy in POLICIES {
                for cap in [None, Some(50)] {
                    if cap.is_some() && inspection != available {
                        assert!(validate(workers, Some(inspection), cap).is_err());
                        continue;
                    }
                    let budget = WorkerBudget::new(workers, Some(inspection), cap, policy);
                    assert_eq!(budget.inspection, inspection);
                    assert_eq!(budget.helpers, available - inspection);
                    assert_eq!(budget.coordinator, usize::from(workers > 1));
                    assert_eq!(
                        budget.inspection + budget.helpers + budget.coordinator,
                        workers
                    );
                }
            }
        }
    }
    for (inspectors, helpers) in [(40, 9), (48, 1), (49, 0), (1, 48)] {
        let budget = WorkerBudget::new(50, Some(inspectors), None, POLICIES[0]);
        assert_eq!(
            (budget.inspection, budget.helpers, budget.coordinator),
            (inspectors, helpers, 1)
        );
    }
}

#[test]
fn ready_large_budgets_cap_helpers_at_32_and_preserve_smaller_defaults() {
    let ready = OwnerDomainWalkPublicationPolicy::Ready;
    let ordered = OwnerDomainWalkPublicationPolicy::Ordered;
    for workers in 1..=64 {
        let expected = WorkerBudget::new(workers, None, None, ordered);
        let actual = WorkerBudget::new(workers, None, None, ready);
        assert_eq!(
            (actual.inspection, actual.helpers, actual.coordinator),
            (expected.inspection, expected.helpers, expected.coordinator),
            "W={workers}: Ready defaults below 65 workers are unchanged"
        );
    }
    for (workers, inspectors) in [(65, 32), (66, 33), (97, 64), (128, 95), (256, 223)] {
        let budget = WorkerBudget::new(workers, None, None, ready);
        assert_eq!(
            (budget.inspection, budget.helpers, budget.coordinator),
            (inspectors, READY_HELPER_CAP, 1),
            "W={workers}"
        );
        assert_eq!(
            budget.inspection + budget.helpers + budget.coordinator,
            workers
        );
        let uncapped = WorkerBudget::new(workers, None, None, ordered);
        assert_eq!(
            uncapped.helpers,
            (workers - 1) / 2,
            "Ordered keeps the half split"
        );
    }
    // Explicit partitions and finite caps are not touched by the Ready cap.
    let explicit = WorkerBudget::new(200, Some(100), None, ready);
    assert_eq!((explicit.inspection, explicit.helpers), (100, 99));
    let capped = WorkerBudget::new(200, None, Some(7), ready);
    assert_eq!((capped.inspection, capped.helpers), (199, 0));
}

/// The Ready launcher keeps H = 256 reserved IDs ahead of the inspectors. The
/// automatic split keeps at least two reserved IDs per inspector through
/// W = 128 (95 inspectors) and at least four through W = 97 (64 inspectors).
#[test]
fn ready_lookahead_covers_inspectors_up_to_128_workers() {
    const H: usize = 256;
    let ready = OwnerDomainWalkPublicationPolicy::Ready;
    let mut max_inspectors = 0;
    for workers in 1..=128 {
        let budget = WorkerBudget::new(workers, None, None, ready);
        max_inspectors = max_inspectors.max(budget.inspection);
        assert!(
            H >= 2 * budget.inspection,
            "W={workers}: {} inspectors exceed half the lookahead",
            budget.inspection
        );
        if workers <= 97 {
            assert!(
                H >= 4 * budget.inspection,
                "W={workers}: {} inspectors exceed a quarter of the lookahead",
                budget.inspection
            );
        }
    }
    assert_eq!(max_inspectors, 95);
    assert_eq!(WorkerBudget::new(97, None, None, ready).inspection * 4, H);
    assert!(WorkerBudget::new(98, None, None, ready).inspection * 4 > H);
}

#[test]
fn malformed_partitions_fail_before_subtraction_or_native_loading() {
    for workers in 0usize..=256 {
        for inspection in [0, workers.saturating_add(1), usize::MAX] {
            assert!(validate(workers, Some(inspection), None).is_err());
        }
    }
    assert!(validate(0, None, None).is_err());
    assert!(validate(2, Some(2), None).is_err());
    assert!(validate(50, Some(40), Some(7)).is_err());
    for (workers, inspection, cap) in [(1, 2, None), (2, 2, None), (50, 0, None), (50, 40, Some(7))]
    {
        let mut request = OwnerDomainWalkRequest::new(super::super::OwnerDomainMatchRequest::new(
            "not JSON".into(),
            "not JSON".into(),
        ));
        request.workers = workers;
        request.inspection_workers = Some(inspection);
        request.max_containment_checks = cap;
        let expected = validate(workers, Some(inspection), cap).unwrap_err();
        let error = super::super::owner_domain_walk_with_progress(
            request,
            &std::sync::atomic::AtomicBool::new(false),
            |_| panic!("before admission"),
        )
        .unwrap_err();
        assert!(error.to_string().contains(expected), "{error}");
    }
}

#[test]
fn allocation_receipts_distinguish_requested_reservations_from_active_workers() {
    for explicit in [None, Some(40)] {
        let budget = WorkerBudget::new(50, explicit, None, POLICIES[0]);
        let allocation = budget.json(explicit);
        assert_eq!(allocation["requested_inspection_workers"], json!(explicit));
        assert_eq!(allocation["total_compute_worker_limit"], 50);
        assert!(allocation.get("active_workers").is_none());
        let progress = super::super::OwnerDomainWalkResult::completion_progress(
            &json!({"worker_allocation":allocation, "requested_inspection_workers":explicit}),
        );
        assert_eq!(progress["worker_allocation"], allocation);
        assert_eq!(progress["requested_inspection_workers"], json!(explicit));
    }
}
