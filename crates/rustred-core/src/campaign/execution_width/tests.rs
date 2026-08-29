use super::super::{
    CampaignBytes, CampaignEstimatorRevision, CampaignMemoryEstimate, CampaignTaskMemoryEnvelope,
    CampaignTaskResourceEstimate,
};
use super::*;

fn memory(revision: CampaignEstimatorRevision, bytes: u64) -> CampaignTaskResourceEstimate {
    let envelope = CampaignTaskMemoryEnvelope::try_new(
        CampaignMemoryEstimate::try_new(CampaignBytes::new(bytes), CampaignBytes::ZERO).unwrap(),
        CampaignMemoryEstimate::zero(),
    )
    .unwrap();
    CampaignTaskResourceEstimate::try_new(revision, 1, envelope).unwrap()
}

fn fixed(non_worker: u64, per_worker: u64) -> CampaignExecutionFixedMemory {
    CampaignExecutionFixedMemory::try_new(
        CampaignBytes::new(non_worker),
        CampaignBytes::ZERO,
        CampaignBytes::new(per_worker),
        CampaignBytes::ZERO,
        CampaignBytes::ZERO,
        CampaignBytes::ZERO,
        CampaignBytes::ZERO,
        CampaignBytes::ZERO,
    )
    .unwrap()
}

fn request(
    requested: usize,
    enclosing: u64,
    operational: u64,
    non_worker: u64,
    per_worker: u64,
    minimum_task: u64,
) -> CampaignExecutionWidthRequest {
    let revision = CampaignEstimatorRevision::try_new(7).unwrap();
    CampaignExecutionWidthRequest::try_new(
        revision,
        requested,
        CampaignBytes::new(enclosing),
        CampaignBytes::new(operational),
        fixed(non_worker, per_worker),
        memory(revision, minimum_task),
    )
    .unwrap()
}

#[test]
fn selects_largest_width_and_records_complete_physical_metadata() {
    let outcome =
        CampaignExecutionWidthPlanner::try_plan(request(100, 1_024, 900, 100, 10, 100)).unwrap();
    let CampaignExecutionWidthPlanningOutcome::Ready(plan) = outcome else {
        panic!("the calibrated request must fit")
    };
    assert_eq!(plan.estimator_revision().get(), 7);
    assert_eq!(plan.requested_core_ceiling(), 100);
    assert_eq!(plan.effective_width(), 70);
    assert_eq!(plan.worker_thread_count(), 70);
    assert_eq!(plan.enclosing_memory_limit(), CampaignBytes::new(1_024));
    assert_eq!(plan.operational_memory_limit(), CampaignBytes::new(900));
    assert_eq!(plan.selected_fixed_memory(), CampaignBytes::new(800));
    assert_eq!(plan.minimum_required_memory(), CampaignBytes::new(900));
    assert_eq!(
        plan.operational_headroom_after_minimum(),
        CampaignBytes::ZERO
    );
    assert_eq!(plan.enclosing_headroom(), CampaignBytes::new(124));
    assert_eq!(plan.baseline_memory().total(), CampaignBytes::new(800));
    assert_eq!(
        plan.fixed_memory().per_worker_stack_tls_workspace(),
        CampaignBytes::new(10)
    );
}

#[test]
fn one_worker_reserve_does_not_create_a_two_worker_pool() {
    let outcome =
        CampaignExecutionWidthPlanner::try_plan(request(100, 500, 250, 100, 100, 50)).unwrap();
    let CampaignExecutionWidthPlanningOutcome::Ready(plan) = outcome else {
        panic!("inline execution must fit")
    };
    assert_eq!(plan.effective_width(), 1);
    assert_eq!(plan.worker_thread_count(), 0);
    assert_eq!(plan.selected_fixed_memory(), CampaignBytes::new(100));
    assert_eq!(plan.minimum_required_memory(), CampaignBytes::new(150));
}

#[test]
fn requested_one_and_zero_worker_reserve_have_exact_semantics() {
    let CampaignExecutionWidthPlanningOutcome::Ready(inline) =
        CampaignExecutionWidthPlanner::try_plan(request(1, 500, 400, 100, 0, 50)).unwrap()
    else {
        panic!("inline request must fit")
    };
    assert_eq!(inline.effective_width(), 1);
    assert_eq!(inline.worker_thread_count(), 0);

    let CampaignExecutionWidthPlanningOutcome::Ready(wide) =
        CampaignExecutionWidthPlanner::try_plan(request(100, 500, 400, 100, 0, 50)).unwrap()
    else {
        panic!("zero worker reserve must admit the requested width")
    };
    assert_eq!(wide.effective_width(), 100);
    assert_eq!(wide.worker_thread_count(), 100);
}

#[test]
fn inline_exact_boundary_fits_and_one_below_returns_typed_pause() {
    let CampaignExecutionWidthPlanningOutcome::Ready(exact) =
        CampaignExecutionWidthPlanner::try_plan(request(8, 301, 300, 200, 80, 100)).unwrap()
    else {
        panic!("the exact inline boundary must fit")
    };
    assert_eq!(exact.effective_width(), 1);
    assert_eq!(exact.minimum_required_memory(), CampaignBytes::new(300));

    let CampaignExecutionWidthPlanningOutcome::PausedForMemoryCapacity(pause) =
        CampaignExecutionWidthPlanner::try_plan(request(8, 301, 299, 200, 80, 100)).unwrap()
    else {
        panic!("one byte below the inline minimum must pause")
    };
    assert_eq!(pause.inline_fixed_memory(), CampaignBytes::new(200));
    assert_eq!(
        pause.inline_minimum_required_memory(),
        CampaignBytes::new(300)
    );
    assert_eq!(pause.memory_shortfall(), CampaignBytes::new(1));
}

#[test]
fn invalid_limits_and_fixed_sum_overflow_are_rejected() {
    let revision = CampaignEstimatorRevision::try_new(1).unwrap();
    let fixed = fixed(1, 1);
    let task = memory(revision, 1);
    assert!(matches!(
        CampaignExecutionWidthRequest::try_new(
            revision,
            0,
            CampaignBytes::new(10),
            CampaignBytes::new(9),
            fixed,
            task,
        ),
        Err(CampaignExecutionWidthError::ZeroRequestedCoreCeiling)
    ));

    let envelope = CampaignTaskMemoryEnvelope::try_new(
        CampaignMemoryEstimate::zero(),
        CampaignMemoryEstimate::zero(),
    )
    .unwrap();
    let wrong_revision = CampaignTaskResourceEstimate::try_new(
        CampaignEstimatorRevision::try_new(2).unwrap(),
        1,
        envelope,
    )
    .unwrap();
    assert!(matches!(
        CampaignExecutionWidthRequest::try_new(
            revision,
            1,
            CampaignBytes::new(10),
            CampaignBytes::new(9),
            fixed,
            wrong_revision,
        ),
        Err(CampaignExecutionWidthError::MinimumTaskEstimatorRevisionMismatch { .. })
    ));
    let wide_task = CampaignTaskResourceEstimate::try_new(revision, 2, envelope).unwrap();
    assert!(matches!(
        CampaignExecutionWidthRequest::try_new(
            revision,
            1,
            CampaignBytes::new(10),
            CampaignBytes::new(9),
            fixed,
            wide_task,
        ),
        Err(CampaignExecutionWidthError::MinimumTaskMustUseOneCore { actual: 2 })
    ));
    for (enclosing, operational) in [(0, 0), (10, 0), (10, 10), (10, 11)] {
        assert!(
            CampaignExecutionWidthRequest::try_new(
                revision,
                1,
                CampaignBytes::new(enclosing),
                CampaignBytes::new(operational),
                fixed,
                task,
            )
            .is_err()
        );
    }
    assert!(matches!(
        CampaignExecutionFixedMemory::try_new(
            CampaignBytes::new(u64::MAX),
            CampaignBytes::new(1),
            CampaignBytes::ZERO,
            CampaignBytes::ZERO,
            CampaignBytes::ZERO,
            CampaignBytes::ZERO,
            CampaignBytes::ZERO,
            CampaignBytes::ZERO,
        ),
        Err(CampaignExecutionWidthError::ByteCountOverflow { .. })
    ));
}

#[test]
fn selected_worker_multiplication_cannot_overflow_or_overadmit() {
    let outcome = CampaignExecutionWidthPlanner::try_plan(request(
        usize::MAX,
        u64::MAX,
        u64::MAX - 1,
        1,
        u64::MAX / 2,
        1,
    ))
    .unwrap();
    let CampaignExecutionWidthPlanningOutcome::Ready(plan) = outcome else {
        panic!("inline execution remains feasible")
    };
    assert_eq!(plan.effective_width(), 1);
    assert_eq!(plan.worker_thread_count(), 0);
    assert!(plan.minimum_required_memory() <= plan.operational_memory_limit());
}
