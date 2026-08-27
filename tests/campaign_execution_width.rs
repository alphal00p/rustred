use rustred::{
    CAMPAIGN_EXECUTION_WIDTH_PLAN_V1_SCHEMA, CampaignAdmissionController, CampaignAdmissionError,
    CampaignBytes, CampaignEstimatorRevision, CampaignExecutionFixedMemory,
    CampaignExecutionWidthPlanner, CampaignExecutionWidthPlanningOutcome,
    CampaignExecutionWidthRequest, CampaignMemoryEstimate, CampaignTaskMemoryEnvelope,
    CampaignTaskResourceEstimate, ParallelExecutionError,
};

fn task(
    revision: CampaignEstimatorRevision,
    retained: u64,
    transient: u64,
) -> CampaignTaskResourceEstimate {
    CampaignTaskResourceEstimate::try_new(
        revision,
        1,
        CampaignTaskMemoryEnvelope::try_new(
            CampaignMemoryEstimate::try_new(CampaignBytes::new(retained), CampaignBytes::ZERO)
                .unwrap(),
            CampaignMemoryEstimate::try_new(CampaignBytes::new(transient), CampaignBytes::ZERO)
                .unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}

fn fixed_with_hydrated(per_worker: u64, hydrated: u64) -> CampaignExecutionFixedMemory {
    CampaignExecutionFixedMemory::try_new(
        CampaignBytes::new(20), // process runtime and shared catalogs
        CampaignBytes::new(10), // coordinator stack/TLS/Workspace
        CampaignBytes::new(per_worker),
        CampaignBytes::new(5), // explicitly admitted inner threads
        CampaignBytes::new(hydrated),
        CampaignBytes::new(10),
        CampaignBytes::new(20),
        CampaignBytes::new(20),
    )
    .unwrap()
}

fn fixed(per_worker: u64) -> CampaignExecutionFixedMemory {
    fixed_with_hydrated(per_worker, 15)
}

#[test]
fn synthetic_width_one_hundred_is_host_independent_and_memory_limited() {
    let revision = CampaignEstimatorRevision::try_new(19).unwrap();
    let request = CampaignExecutionWidthRequest::try_new(
        revision,
        100,
        CampaignBytes::new(1_024),
        CampaignBytes::new(900),
        fixed(10),
        task(revision, 60, 40),
    )
    .unwrap();
    let CampaignExecutionWidthPlanningOutcome::Ready(plan) =
        CampaignExecutionWidthPlanner::try_plan(request).unwrap()
    else {
        panic!("the synthetic EPYC envelope must admit progress")
    };

    assert_eq!(plan.schema(), CAMPAIGN_EXECUTION_WIDTH_PLAN_V1_SCHEMA);
    assert_eq!(plan.requested_core_ceiling(), 100);
    assert_eq!(plan.effective_width(), 70);
    assert_eq!(plan.worker_thread_count(), 70);
    assert_eq!(plan.selected_fixed_memory(), CampaignBytes::new(800));
    assert_eq!(plan.minimum_required_memory(), CampaignBytes::new(900));
    assert_eq!(
        plan.admission_baseline().fixed_and_shared(),
        CampaignBytes::new(775)
    );
    assert_eq!(
        plan.admission_baseline().hydrated_retained(),
        CampaignBytes::new(15)
    );
    assert_eq!(
        plan.admission_baseline().staged_results(),
        CampaignBytes::new(10)
    );
}

#[test]
fn no_fit_is_a_typed_pause_and_cannot_construct_an_executor() {
    let revision = CampaignEstimatorRevision::try_new(23).unwrap();
    let request = CampaignExecutionWidthRequest::try_new(
        revision,
        100,
        CampaignBytes::new(301),
        CampaignBytes::new(299),
        fixed(10),
        task(revision, 100, 100),
    )
    .unwrap();
    let CampaignExecutionWidthPlanningOutcome::PausedForMemoryCapacity(pause) =
        CampaignExecutionWidthPlanner::try_plan(request).unwrap()
    else {
        panic!("the inline minimum exceeds the operational envelope")
    };
    assert_eq!(pause.inline_fixed_memory(), CampaignBytes::new(100));
    assert_eq!(
        pause.inline_minimum_required_memory(),
        CampaignBytes::new(300)
    );
    assert_eq!(pause.memory_shortfall(), CampaignBytes::new(1));
}

#[test]
fn accepted_inline_plan_consumes_without_creating_a_rayon_worker() {
    let revision = CampaignEstimatorRevision::try_new(29).unwrap();
    let request = CampaignExecutionWidthRequest::try_new(
        revision,
        1,
        CampaignBytes::new(1_024),
        CampaignBytes::new(900),
        fixed_with_hydrated(10, 0),
        task(revision, 60, 40),
    )
    .unwrap();
    let CampaignExecutionWidthPlanningOutcome::Ready(plan) =
        CampaignExecutionWidthPlanner::try_plan(request).unwrap()
    else {
        panic!("inline execution must fit")
    };
    let controller = CampaignAdmissionController::try_from_execution_width_plan(plan).unwrap();
    assert_eq!(controller.try_usage().unwrap().core_capacity(), 1);
    assert_eq!(controller.worker_thread_count(), 0);
    assert!(!controller.is_parallel());
}

#[test]
fn accepted_parallel_plan_constructs_exactly_the_effective_worker_count() {
    if !symbolica::LicenseManager::is_licensed()
        || std::thread::available_parallelism().unwrap().get() < 2
    {
        return;
    }
    let revision = CampaignEstimatorRevision::try_new(31).unwrap();
    let request = CampaignExecutionWidthRequest::try_new(
        revision,
        2,
        CampaignBytes::new(1_024),
        CampaignBytes::new(900),
        fixed_with_hydrated(10, 0),
        task(revision, 60, 40),
    )
    .unwrap();
    let CampaignExecutionWidthPlanningOutcome::Ready(plan) =
        CampaignExecutionWidthPlanner::try_plan(request).unwrap()
    else {
        panic!("two workers must fit")
    };
    let controller = CampaignAdmissionController::try_from_execution_width_plan(plan).unwrap();
    assert_eq!(
        controller.execution_width_plan().unwrap().effective_width(),
        2
    );
    assert_eq!(controller.try_usage().unwrap().core_capacity(), 2);
    assert_eq!(controller.worker_thread_count(), 2);
    assert!(controller.is_parallel());
}

#[test]
fn plan_consuming_admission_bootstrap_cannot_erase_warmed_execution_memory() {
    let revision = CampaignEstimatorRevision::try_new(37).unwrap();
    let request = CampaignExecutionWidthRequest::try_new(
        revision,
        1,
        CampaignBytes::new(1_024),
        CampaignBytes::new(900),
        fixed_with_hydrated(10, 0),
        task(revision, 60, 40),
    )
    .unwrap();
    let CampaignExecutionWidthPlanningOutcome::Ready(plan) =
        CampaignExecutionWidthPlanner::try_plan(request).unwrap()
    else {
        panic!("inline execution must fit")
    };
    let planned_fixed = plan.admission_baseline().fixed_and_shared();
    let planned_staged = plan.admission_baseline().staged_results();
    let mut controller = CampaignAdmissionController::try_from_execution_width_plan(plan).unwrap();

    let retained_plan = controller.execution_width_plan().unwrap();
    assert_eq!(retained_plan.effective_width(), 1);
    let usage = controller.try_usage().unwrap();
    assert_eq!(usage.core_capacity(), 1);
    assert_eq!(usage.max_memory(), CampaignBytes::new(900));
    assert_eq!(usage.baseline().fixed_and_shared(), planned_fixed);
    assert_eq!(usage.baseline().staged_results(), planned_staged);

    controller
        .try_set_fixed_and_shared(CampaignBytes::new(5))
        .unwrap();
    assert_eq!(
        controller
            .try_usage()
            .unwrap()
            .baseline()
            .fixed_and_shared(),
        CampaignBytes::new(planned_fixed.get() + 5)
    );
    controller
        .try_set_fixed_and_shared(CampaignBytes::ZERO)
        .unwrap();
    assert_eq!(
        controller
            .try_usage()
            .unwrap()
            .baseline()
            .fixed_and_shared(),
        planned_fixed
    );
}

#[test]
fn admission_bootstrap_rejects_unowned_hydrated_bytes_before_pool_creation() {
    let revision = CampaignEstimatorRevision::try_new(41).unwrap();
    let request = CampaignExecutionWidthRequest::try_new(
        revision,
        2,
        CampaignBytes::new(1_024),
        CampaignBytes::new(900),
        fixed_with_hydrated(10, 15),
        task(revision, 60, 40),
    )
    .unwrap();
    let CampaignExecutionWidthPlanningOutcome::Ready(plan) =
        CampaignExecutionWidthPlanner::try_plan(request).unwrap()
    else {
        panic!("the memory plan itself must fit")
    };
    assert!(matches!(
        CampaignAdmissionController::try_from_execution_width_plan(plan),
        Err(CampaignAdmissionError::ExecutionWidthPlanHasHydratedRetainedMemory {
            bytes
        }) if bytes == CampaignBytes::new(15)
    ));
}

#[test]
fn requested_host_ceiling_is_validated_even_when_memory_shrinks_effective_width_to_one() {
    let available = std::thread::available_parallelism().unwrap().get();
    let requested = available.checked_add(1).unwrap();
    let revision = CampaignEstimatorRevision::try_new(43).unwrap();
    let request = CampaignExecutionWidthRequest::try_new(
        revision,
        requested,
        CampaignBytes::new(1_024),
        CampaignBytes::new(900),
        fixed_with_hydrated(1_000, 0),
        task(revision, 60, 40),
    )
    .unwrap();
    let CampaignExecutionWidthPlanningOutcome::Ready(plan) =
        CampaignExecutionWidthPlanner::try_plan(request).unwrap()
    else {
        panic!("inline progress must remain memory-feasible")
    };
    assert_eq!(plan.effective_width(), 1);
    assert!(matches!(
        CampaignAdmissionController::try_from_execution_width_plan(plan),
        Err(CampaignAdmissionError::ParallelExecution(
            ParallelExecutionError::CoreBudgetExceedsAvailable {
                requested: actual_requested,
                available: actual_available,
            }
        )) if actual_requested == requested && actual_available == available
    ));
}
