use super::*;

const POLICIES: [OwnerDomainWalkPublicationPolicy; 3] = [
    OwnerDomainWalkPublicationPolicy::Ordered,
    OwnerDomainWalkPublicationPolicy::OwnerBatched,
    OwnerDomainWalkPublicationPolicy::Ready,
];

#[test]
fn automatic_worker_partitions_preserve_both_historical_defaults() {
    for workers in 1..=64 {
        for policy in POLICIES {
            for cap in [None, Some(50)] {
                let budget = WorkerBudget::new(workers, None, cap, policy);
                let threshold = if policy != OwnerDomainWalkPublicationPolicy::OwnerBatched {
                    5
                } else {
                    4
                };
                let helpers = if workers >= threshold && cap.is_none() {
                    (workers - 1) / 2
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
    for workers in 1..=64 {
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
fn malformed_partitions_fail_before_subtraction_or_native_loading() {
    for workers in 0usize..=64 {
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
