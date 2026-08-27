use std::collections::BTreeMap;
use std::sync::Arc;

use rustred::{
    AffineDenominator, CampaignBaselineMemory, CampaignBytes, CampaignEstimatorRevision,
    CampaignMemoryEstimate, CampaignPlan, CampaignPlanLimits, CampaignResourceError,
    CampaignResourcePolicy, CampaignRootSpec, CampaignTaskMemoryEnvelope,
    CampaignTaskResourceEstimate, CampaignWavePlanner, CoefficientContext, IntegralFamily,
    IntegralOrderingPolicy, SectorMask,
};

fn family() -> Arc<IntegralFamily> {
    let coefficients = CoefficientContext::new(["d"]);
    Arc::new(
        IntegralFamily::new(
            "campaign-resource-family",
            vec!["k1".into(), "k2".into()],
            Vec::new(),
            coefficients.clone(),
            coefficients.parameter("d").unwrap(),
            vec![
                AffineDenominator::new(
                    coefficients.zero(),
                    vec![coefficients.one(), coefficients.zero(), coefficients.zero()],
                ),
                AffineDenominator::new(
                    coefficients.zero(),
                    vec![coefficients.zero(), coefficients.one(), coefficients.zero()],
                ),
                AffineDenominator::new(
                    coefficients.zero(),
                    vec![coefficients.zero(), coefficients.zero(), coefficients.one()],
                ),
            ],
            Vec::new(),
            vec![
                coefficients.zero(),
                coefficients.zero(),
                coefficients.zero(),
            ],
        )
        .unwrap(),
    )
}

fn jobs() -> Vec<rustred::CampaignJobKey> {
    let family = family();
    let roots = ["100", "010", "001"]
        .into_iter()
        .enumerate()
        .map(|(ordinal, sector)| {
            CampaignRootSpec::try_new(
                format!("root-{ordinal}"),
                Arc::clone(&family),
                SectorMask::try_from_bit_string(sector).unwrap(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    CampaignPlan::compile(
        roots,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        CampaignPlanLimits::default(),
    )
    .unwrap()
    .intrinsic_jobs()
    .cloned()
    .collect()
}

fn wide_jobs(count: usize) -> Vec<rustred::CampaignJobKey> {
    assert!(count <= 1_023);
    let coefficients = CoefficientContext::new(["d"]);
    let scalar_products = 10usize;
    let denominators = (0..scalar_products)
        .map(|row| {
            let mut entries = vec![coefficients.zero(); scalar_products];
            entries[row] = coefficients.one();
            AffineDenominator::new(coefficients.zero(), entries)
        })
        .collect();
    let family = Arc::new(
        IntegralFamily::new(
            "campaign-resource-wide-family",
            vec!["k1".into(), "k2".into(), "k3".into(), "k4".into()],
            Vec::new(),
            coefficients.clone(),
            coefficients.parameter("d").unwrap(),
            denominators,
            Vec::new(),
            vec![coefficients.zero(); scalar_products],
        )
        .unwrap(),
    );
    let roots = (1..=count)
        .map(|ordinal| {
            CampaignRootSpec::try_new(
                format!("wide-root-{ordinal:03}"),
                Arc::clone(&family),
                SectorMask::try_from_bit_string(&format!("{ordinal:010b}")).unwrap(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    CampaignPlan::compile(
        roots,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        CampaignPlanLimits::default(),
    )
    .unwrap()
    .intrinsic_jobs()
    .cloned()
    .collect()
}

fn memory(retained: u64, transient: u64) -> CampaignTaskMemoryEnvelope {
    CampaignTaskMemoryEnvelope::try_new(
        CampaignMemoryEstimate::try_new(CampaignBytes::new(retained), CampaignBytes::ZERO).unwrap(),
        CampaignMemoryEstimate::try_new(CampaignBytes::new(transient), CampaignBytes::ZERO)
            .unwrap(),
    )
    .unwrap()
}

#[test]
fn stable_first_fit_leaves_cores_idle_when_memory_is_the_bottleneck() {
    let jobs = jobs();
    let revision = CampaignEstimatorRevision::try_new(1).unwrap();
    let baseline = CampaignBaselineMemory::try_new(
        CampaignBytes::new(300),
        CampaignBytes::ZERO,
        CampaignBytes::ZERO,
    )
    .unwrap();
    let policy =
        CampaignResourcePolicy::try_new(revision, 100, CampaignBytes::new(1_000), baseline)
            .unwrap();
    let mut requests = BTreeMap::new();
    requests.insert(
        jobs[0].clone(),
        CampaignTaskResourceEstimate::try_new(revision, 1, memory(300, 300)).unwrap(),
    );
    requests.insert(
        jobs[1].clone(),
        CampaignTaskResourceEstimate::try_new(revision, 1, memory(150, 150)).unwrap(),
    );
    requests.insert(
        jobs[2].clone(),
        CampaignTaskResourceEstimate::try_new(revision, 1, memory(150, 150)).unwrap(),
    );
    let wave = CampaignWavePlanner::try_plan(policy, &requests).unwrap();
    assert_eq!(wave.jobs(), &[jobs[0].clone()]);
    assert_eq!(wave.selected_cores(), 1);
    assert_eq!(
        wave.selected_peak_additional_memory(),
        CampaignBytes::new(600)
    );
}

#[test]
fn wave_is_independent_of_request_insertion_order() {
    let jobs = jobs();
    let revision = CampaignEstimatorRevision::try_new(7).unwrap();
    let baseline = CampaignBaselineMemory::try_new(
        CampaignBytes::new(100),
        CampaignBytes::new(100),
        CampaignBytes::new(100),
    )
    .unwrap();
    let policy =
        CampaignResourcePolicy::try_new(revision, 2, CampaignBytes::new(1_000), baseline).unwrap();
    let estimate = CampaignTaskResourceEstimate::try_new(revision, 1, memory(100, 100)).unwrap();
    let left = BTreeMap::from([
        (jobs[2].clone(), estimate),
        (jobs[0].clone(), estimate),
        (jobs[1].clone(), estimate),
    ]);
    let right = BTreeMap::from([
        (jobs[1].clone(), estimate),
        (jobs[2].clone(), estimate),
        (jobs[0].clone(), estimate),
    ]);
    assert_eq!(
        CampaignWavePlanner::try_plan(policy, &left).unwrap(),
        CampaignWavePlanner::try_plan(policy, &right).unwrap()
    );
}

#[test]
fn individually_oversized_task_pauses_as_a_typed_error() {
    let job = jobs().remove(0);
    let revision = CampaignEstimatorRevision::try_new(1).unwrap();
    let baseline = CampaignBaselineMemory::try_new(
        CampaignBytes::new(400),
        CampaignBytes::ZERO,
        CampaignBytes::ZERO,
    )
    .unwrap();
    let policy =
        CampaignResourcePolicy::try_new(revision, 100, CampaignBytes::new(1_000), baseline)
            .unwrap();
    let requests = BTreeMap::from([(
        job,
        CampaignTaskResourceEstimate::try_new(revision, 1, memory(400, 300)).unwrap(),
    )]);
    assert!(matches!(
        CampaignWavePlanner::try_plan(policy, &requests),
        Err(CampaignResourceError::TaskMemoryRequestExceedsCapacity {
            baseline,
            additional,
            capacity,
            ..
        }) if baseline == CampaignBytes::new(400)
            && additional == CampaignBytes::new(700)
            && capacity == CampaignBytes::new(1_000)
    ));
}

#[test]
fn oversized_jobs_are_skipped_while_an_admissible_wave_exists() {
    let jobs = jobs();
    let revision = CampaignEstimatorRevision::try_new(1).unwrap();
    let baseline = CampaignBaselineMemory::try_new(
        CampaignBytes::new(300),
        CampaignBytes::ZERO,
        CampaignBytes::ZERO,
    )
    .unwrap();
    let policy =
        CampaignResourcePolicy::try_new(revision, 2, CampaignBytes::new(1_000), baseline).unwrap();
    let requests = BTreeMap::from([
        (
            jobs[0].clone(),
            CampaignTaskResourceEstimate::try_new(revision, 1, memory(800, 0)).unwrap(),
        ),
        (
            jobs[1].clone(),
            CampaignTaskResourceEstimate::try_new(revision, 1, memory(100, 100)).unwrap(),
        ),
        (
            jobs[2].clone(),
            CampaignTaskResourceEstimate::try_new(revision, 1, memory(100, 100)).unwrap(),
        ),
    ]);
    let wave = CampaignWavePlanner::try_plan(policy, &requests).unwrap();
    assert_eq!(wave.jobs(), &[jobs[1].clone(), jobs[2].clone()]);
    assert_eq!(wave.selected_cores(), 2);
    assert_eq!(
        wave.selected_peak_additional_memory(),
        CampaignBytes::new(400)
    );
}

#[test]
fn resource_policy_rejects_revision_core_and_checked_byte_mismatches() {
    assert!(matches!(
        CampaignEstimatorRevision::try_new(0),
        Err(CampaignResourceError::ZeroEstimatorRevision)
    ));
    assert_eq!(
        CampaignEstimatorRevision::try_new(u64::MAX).unwrap().get(),
        u64::MAX
    );
    assert!(matches!(
        CampaignMemoryEstimate::try_new(CampaignBytes::new(u64::MAX), CampaignBytes::new(1)),
        Err(CampaignResourceError::ByteCountOverflow { .. })
    ));
    let job = jobs().remove(0);
    let revision = CampaignEstimatorRevision::try_new(1).unwrap();
    let other_revision = CampaignEstimatorRevision::try_new(2).unwrap();
    let baseline = CampaignBaselineMemory::try_new(
        CampaignBytes::new(1),
        CampaignBytes::ZERO,
        CampaignBytes::ZERO,
    )
    .unwrap();
    let policy =
        CampaignResourcePolicy::try_new(revision, 2, CampaignBytes::new(1_000), baseline).unwrap();
    let mismatch = BTreeMap::from([(
        job.clone(),
        CampaignTaskResourceEstimate::try_new(other_revision, 1, memory(1, 1)).unwrap(),
    )]);
    assert!(matches!(
        CampaignWavePlanner::try_plan(policy, &mismatch),
        Err(CampaignResourceError::EstimatorRevisionMismatch { .. })
    ));
    let too_many_cores = BTreeMap::from([(
        job,
        CampaignTaskResourceEstimate::try_new(revision, 3, memory(1, 1)).unwrap(),
    )]);
    assert!(matches!(
        CampaignWavePlanner::try_plan(policy, &too_many_cores),
        Err(CampaignResourceError::TaskCoreRequestExceedsCapacity { .. })
    ));
}

#[test]
fn hundred_core_one_tibibyte_wave_keeps_idle_cores_under_ram_pressure() {
    const GIB: u64 = 1 << 30;
    let jobs = wide_jobs(100);
    let revision = CampaignEstimatorRevision::try_new(1).unwrap();
    let baseline = CampaignBaselineMemory::try_new(
        CampaignBytes::new(100 * GIB),
        CampaignBytes::ZERO,
        CampaignBytes::ZERO,
    )
    .unwrap();
    let policy =
        CampaignResourcePolicy::try_new(revision, 100, CampaignBytes::new(1 << 40), baseline)
            .unwrap();
    let estimate =
        CampaignTaskResourceEstimate::try_new(revision, 1, memory(8 * GIB, 8 * GIB)).unwrap();
    let requests = jobs
        .into_iter()
        .map(|job| (job, estimate))
        .collect::<BTreeMap<_, _>>();

    let wave = CampaignWavePlanner::try_plan(policy, &requests).unwrap();
    assert_eq!(wave.jobs().len(), 57);
    assert_eq!(wave.selected_cores(), 57);
    assert_eq!(
        wave.selected_peak_additional_memory(),
        CampaignBytes::new(912 * GIB)
    );
    assert!(wave.jobs().len() <= policy.cores());
    assert!(
        policy.baseline().total().get() + wave.selected_peak_additional_memory().get()
            <= policy.max_memory().get()
    );

    // A production ceiling must normally sit below physical RAM. With a
    // 900-GiB operational envelope on the same nominal 1-TiB host, stable
    // admission activates only 50 cores and keeps the other 50 idle.
    let headroom_policy =
        CampaignResourcePolicy::try_new(revision, 100, CampaignBytes::new(900 * GIB), baseline)
            .unwrap();
    let headroom_wave = CampaignWavePlanner::try_plan(headroom_policy, &requests).unwrap();
    assert_eq!(headroom_wave.jobs().len(), 50);
    assert_eq!(headroom_wave.selected_cores(), 50);
    assert_eq!(
        headroom_wave.selected_peak_additional_memory(),
        CampaignBytes::new(800 * GIB)
    );
}

#[test]
fn selection_handles_empty_exact_boundary_and_aggregate_u64_nonfit() {
    let jobs = jobs();
    let revision = CampaignEstimatorRevision::try_new(9).unwrap();
    let empty_policy = CampaignResourcePolicy::try_new(
        revision,
        1,
        CampaignBytes::new(1),
        CampaignBaselineMemory::try_new(
            CampaignBytes::ZERO,
            CampaignBytes::ZERO,
            CampaignBytes::ZERO,
        )
        .unwrap(),
    )
    .unwrap();
    let empty = CampaignWavePlanner::try_plan(empty_policy, &BTreeMap::new()).unwrap();
    assert!(empty.is_empty());

    let exact_policy = CampaignResourcePolicy::try_new(
        revision,
        4,
        CampaignBytes::new(500),
        CampaignBaselineMemory::try_new(
            CampaignBytes::new(100),
            CampaignBytes::ZERO,
            CampaignBytes::ZERO,
        )
        .unwrap(),
    )
    .unwrap();
    let exact_requests = BTreeMap::from([
        (
            jobs[0].clone(),
            CampaignTaskResourceEstimate::try_new(revision, 2, memory(100, 100)).unwrap(),
        ),
        (
            jobs[1].clone(),
            CampaignTaskResourceEstimate::try_new(revision, 2, memory(100, 100)).unwrap(),
        ),
    ]);
    let exact = CampaignWavePlanner::try_plan(exact_policy, &exact_requests).unwrap();
    assert_eq!(exact.jobs().len(), 2);
    assert_eq!(exact.selected_cores(), 4);
    assert_eq!(
        exact.selected_peak_additional_memory(),
        CampaignBytes::new(400)
    );

    let huge_policy = CampaignResourcePolicy::try_new(
        revision,
        2,
        CampaignBytes::new(u64::MAX),
        CampaignBaselineMemory::try_new(
            CampaignBytes::ZERO,
            CampaignBytes::ZERO,
            CampaignBytes::ZERO,
        )
        .unwrap(),
    )
    .unwrap();
    let huge_requests = BTreeMap::from([
        (
            jobs[0].clone(),
            CampaignTaskResourceEstimate::try_new(revision, 1, memory(u64::MAX - 5, 0)).unwrap(),
        ),
        (
            jobs[1].clone(),
            CampaignTaskResourceEstimate::try_new(revision, 1, memory(10, 0)).unwrap(),
        ),
    ]);
    let huge = CampaignWavePlanner::try_plan(huge_policy, &huge_requests).unwrap();
    assert_eq!(huge.jobs(), &[jobs[0].clone()]);
}

#[test]
fn core_oversized_job_is_skipped_until_it_is_the_only_remaining_work() {
    let jobs = jobs();
    let revision = CampaignEstimatorRevision::try_new(1).unwrap();
    let policy = CampaignResourcePolicy::try_new(
        revision,
        2,
        CampaignBytes::new(1_000),
        CampaignBaselineMemory::try_new(
            CampaignBytes::ZERO,
            CampaignBytes::ZERO,
            CampaignBytes::ZERO,
        )
        .unwrap(),
    )
    .unwrap();
    let mixed = BTreeMap::from([
        (
            jobs[0].clone(),
            CampaignTaskResourceEstimate::try_new(revision, 3, memory(1, 1)).unwrap(),
        ),
        (
            jobs[1].clone(),
            CampaignTaskResourceEstimate::try_new(revision, 1, memory(1, 1)).unwrap(),
        ),
    ]);
    assert_eq!(
        CampaignWavePlanner::try_plan(policy, &mixed)
            .unwrap()
            .jobs(),
        &[jobs[1].clone()]
    );
    let impossible = BTreeMap::from([(
        jobs[0].clone(),
        CampaignTaskResourceEstimate::try_new(revision, 3, memory(1, 1)).unwrap(),
    )]);
    assert!(matches!(
        CampaignWavePlanner::try_plan(policy, &impossible),
        Err(CampaignResourceError::TaskCoreRequestExceedsCapacity {
            requested: 3,
            capacity: 2,
            ..
        })
    ));
}
