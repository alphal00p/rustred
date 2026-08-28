use rustred::campaign::{
    CAMPAIGN_EXECUTION_RESOURCE_PROFILE_V1_SCHEMA, CAMPAIGN_EXECUTION_WIDTH_PLAN_V1_SCHEMA,
    CampaignBytes, CampaignEstimatorRevision, CampaignExecutionFixedMemory,
    CampaignExecutionResourceProfile, CampaignExecutionWidthError, CampaignExecutionWidthPlanner,
    CampaignExecutionWidthPlanningOutcome, CampaignMemoryEstimate, CampaignTaskMemoryEnvelope,
    CampaignTaskResourceEstimate,
};

fn fixed(per_worker: u64) -> CampaignExecutionFixedMemory {
    CampaignExecutionFixedMemory::try_new(
        CampaignBytes::new(20),
        CampaignBytes::new(10),
        CampaignBytes::new(per_worker),
        CampaignBytes::new(5),
        CampaignBytes::ZERO,
        CampaignBytes::new(10),
        CampaignBytes::new(20),
        CampaignBytes::new(20),
    )
    .unwrap()
}

fn task(revision: CampaignEstimatorRevision) -> CampaignTaskResourceEstimate {
    CampaignTaskResourceEstimate::try_new(
        revision,
        1,
        CampaignTaskMemoryEnvelope::try_new(
            CampaignMemoryEstimate::try_new(CampaignBytes::new(60), CampaignBytes::new(10))
                .unwrap(),
            CampaignMemoryEstimate::try_new(CampaignBytes::new(20), CampaignBytes::new(10))
                .unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn explicit_profile_constructs_the_existing_checked_width_request() {
    let revision = CampaignEstimatorRevision::try_new(19).unwrap();
    let fixed = fixed(10);
    let minimum = task(revision);
    let profile = CampaignExecutionResourceProfile::try_new(revision, fixed, minimum).unwrap();

    assert_eq!(
        profile.schema(),
        CAMPAIGN_EXECUTION_RESOURCE_PROFILE_V1_SCHEMA
    );
    assert_eq!(profile.estimator_revision(), revision);
    assert_eq!(profile.fixed_memory(), fixed);
    assert_eq!(profile.minimum_runnable_task(), minimum);

    let request = profile
        .try_into_width_request(100, CampaignBytes::new(1_024), CampaignBytes::new(900))
        .unwrap();
    assert_eq!(request.estimator_revision(), revision);
    assert_eq!(request.requested_core_ceiling(), 100);
    assert_eq!(request.enclosing_memory_limit(), CampaignBytes::new(1_024));
    assert_eq!(request.operational_memory_limit(), CampaignBytes::new(900));
    assert_eq!(request.fixed_memory(), fixed);
    assert_eq!(request.minimum_runnable_task(), minimum);

    let CampaignExecutionWidthPlanningOutcome::Ready(plan) =
        CampaignExecutionWidthPlanner::try_plan(request).unwrap()
    else {
        panic!("the explicit synthetic profile must admit progress")
    };
    assert_eq!(plan.schema(), CAMPAIGN_EXECUTION_WIDTH_PLAN_V1_SCHEMA);
    assert_eq!(plan.effective_width(), 71);
    assert_eq!(plan.worker_thread_count(), 71);
    assert_eq!(plan.minimum_required_memory(), CampaignBytes::new(895));
}

#[test]
fn profile_preserves_exact_fit_and_one_byte_below_planning_boundaries() {
    let revision = CampaignEstimatorRevision::try_new(23).unwrap();
    let profile =
        CampaignExecutionResourceProfile::try_new(revision, fixed(10), task(revision)).unwrap();

    let exact = profile
        .try_into_width_request(8, CampaignBytes::new(186), CampaignBytes::new(185))
        .unwrap();
    let CampaignExecutionWidthPlanningOutcome::Ready(exact) =
        CampaignExecutionWidthPlanner::try_plan(exact).unwrap()
    else {
        panic!("exact equality must fit")
    };
    assert_eq!(exact.effective_width(), 1);
    assert_eq!(exact.worker_thread_count(), 0);
    assert_eq!(exact.minimum_required_memory(), CampaignBytes::new(185));

    let below = profile
        .try_into_width_request(8, CampaignBytes::new(185), CampaignBytes::new(184))
        .unwrap();
    let CampaignExecutionWidthPlanningOutcome::PausedForMemoryCapacity(below) =
        CampaignExecutionWidthPlanner::try_plan(below).unwrap()
    else {
        panic!("one byte below the inline minimum must pause")
    };
    assert_eq!(
        below.inline_minimum_required_memory(),
        CampaignBytes::new(185)
    );
    assert_eq!(below.memory_shortfall(), CampaignBytes::new(1));
}

#[test]
fn profile_delegates_operator_limit_validation_to_the_width_request() {
    let revision = CampaignEstimatorRevision::try_new(29).unwrap();
    let profile =
        CampaignExecutionResourceProfile::try_new(revision, fixed(0), task(revision)).unwrap();

    assert_eq!(
        profile.try_into_width_request(4, CampaignBytes::new(500), CampaignBytes::new(500),),
        Err(
            CampaignExecutionWidthError::OperationalMemoryNotBelowEnclosing {
                operational: CampaignBytes::new(500),
                enclosing: CampaignBytes::new(500),
            }
        )
    );
    assert_eq!(
        profile.try_into_width_request(0, CampaignBytes::new(500), CampaignBytes::new(400),),
        Err(CampaignExecutionWidthError::ZeroRequestedCoreCeiling)
    );
}
