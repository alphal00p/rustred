use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind, panic_any};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, mpsc};
use std::time::Duration;

use rustred::{
    AffineDenominator, CampaignAdmissionController, CampaignAdmissionError, CampaignBytes,
    CampaignEstimatorRevision, CampaignExecutionFixedMemory, CampaignExecutionWidthPlanner,
    CampaignExecutionWidthPlanningOutcome, CampaignExecutionWidthRequest, CampaignMemoryEstimate,
    CampaignPlan, CampaignPlanLimits, CampaignResident, CampaignResidentToken,
    CampaignResidentTransformBuildFailure, CampaignResidentTransformExecution, CampaignRootSpec,
    CampaignTaskExecution, CampaignTaskMemoryEnvelope, CampaignTaskResourceEstimate,
    CampaignWavePlanner, CampaignWorkKey, CoefficientContext, IntegralFamily,
    IntegralOrderingPolicy, SectorMask,
};

const REVISION: u64 = 1;

fn jobs(count: usize) -> Vec<CampaignWorkKey> {
    assert!((1..=7).contains(&count));
    let coefficients = CoefficientContext::new(["d"]);
    let family = Arc::new(
        IntegralFamily::new(
            "campaign-admission-family",
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
    );
    let roots = (1..=count)
        .map(|ordinal| {
            CampaignRootSpec::try_new(
                format!("root-{ordinal}"),
                Arc::clone(&family),
                SectorMask::try_from_bit_string(&format!("{ordinal:03b}")).unwrap(),
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
    .take(count)
    .enumerate()
    .map(|(lane, job)| {
        CampaignWorkKey::job_lane(job.clone(), "campaign-admission-context", lane as u64)
    })
    .collect()
}

fn controller(cores: usize, max_memory: u64, fixed: u64) -> CampaignAdmissionController {
    let revision = CampaignEstimatorRevision::try_new(REVISION).unwrap();
    let fixed_memory = CampaignExecutionFixedMemory::try_new(
        CampaignBytes::new(fixed),
        CampaignBytes::ZERO,
        CampaignBytes::ZERO,
        CampaignBytes::ZERO,
        CampaignBytes::ZERO,
        CampaignBytes::ZERO,
        CampaignBytes::ZERO,
        CampaignBytes::ZERO,
    )
    .unwrap();
    let minimum_task = CampaignTaskResourceEstimate::try_new(
        revision,
        1,
        CampaignTaskMemoryEnvelope::try_new(
            CampaignMemoryEstimate::zero(),
            CampaignMemoryEstimate::zero(),
        )
        .unwrap(),
    )
    .unwrap();
    let request = CampaignExecutionWidthRequest::try_new(
        revision,
        cores,
        CampaignBytes::new(max_memory.checked_add(1).unwrap()),
        CampaignBytes::new(max_memory),
        fixed_memory,
        minimum_task,
    )
    .unwrap();
    let CampaignExecutionWidthPlanningOutcome::Ready(plan) =
        CampaignExecutionWidthPlanner::try_plan(request).unwrap()
    else {
        panic!("test controller baseline must fit")
    };
    assert_eq!(plan.effective_width(), cores);
    CampaignAdmissionController::try_from_execution_width_plan(plan).unwrap()
}

fn memory(retained: u64, transient: u64) -> CampaignTaskMemoryEnvelope {
    CampaignTaskMemoryEnvelope::try_new(
        CampaignMemoryEstimate::try_new(CampaignBytes::new(retained), CampaignBytes::ZERO).unwrap(),
        CampaignMemoryEstimate::try_new(CampaignBytes::new(transient), CampaignBytes::ZERO)
            .unwrap(),
    )
    .unwrap()
}

fn estimate(cores: usize, retained: u64, transient: u64) -> CampaignTaskResourceEstimate {
    CampaignTaskResourceEstimate::try_new(
        CampaignEstimatorRevision::try_new(REVISION).unwrap(),
        cores,
        memory(retained, transient),
    )
    .unwrap()
}

fn one_request(
    work: &CampaignWorkKey,
    retained: u64,
    transient: u64,
) -> BTreeMap<CampaignWorkKey, CampaignTaskResourceEstimate> {
    BTreeMap::from([(work.clone(), estimate(1, retained, transient))])
}

fn reserve_one(
    controller: &mut CampaignAdmissionController,
    work: &CampaignWorkKey,
    retained: u64,
    transient: u64,
    predecessor: Option<CampaignResidentToken>,
) -> rustred::CampaignTaskReservation {
    let snapshot = controller.try_snapshot().unwrap();
    let requests = one_request(work, retained, transient);
    let plan = CampaignWavePlanner::try_plan(snapshot.policy(), &requests).unwrap();
    let predecessors = predecessor
        .map(|token| BTreeMap::from([(work.clone(), token)]))
        .unwrap_or_default();
    controller
        .try_reserve_wave_with_predecessors(&snapshot, &plan, &requests, &predecessors)
        .unwrap()
        .into_tasks()
        .pop()
        .unwrap()
}

fn commit_initial<T>(
    controller: &mut CampaignAdmissionController,
    work: &CampaignWorkKey,
    retained: u64,
    owner: T,
) -> CampaignResident<T> {
    reserve_one(controller, work, retained, 0, None)
        .bind(owner)
        .try_commit_initial()
        .unwrap()
}

#[derive(Clone)]
struct CountedOwner {
    id: &'static str,
    drops: Arc<AtomicUsize>,
}

impl Drop for CountedOwner {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

struct BlockingOwner {
    entered: mpsc::SyncSender<()>,
    release: mpsc::Receiver<()>,
    drops: Arc<AtomicUsize>,
}

impl Drop for BlockingOwner {
    fn drop(&mut self) {
        self.entered.send(()).unwrap();
        self.release.recv().unwrap();
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

struct PanickingOwner {
    drops: Arc<AtomicUsize>,
}

impl Drop for PanickingOwner {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
        panic!("injected retained-owner destructor panic");
    }
}

#[test]
fn whole_wave_is_atomic_and_old_snapshots_cannot_be_replayed() {
    let jobs = jobs(3);
    let mut controller = controller(2, 500, 100);
    let snapshot = controller.try_snapshot().unwrap();
    let requests = BTreeMap::from([
        (jobs[0].clone(), estimate(1, 100, 50)),
        (jobs[1].clone(), estimate(1, 100, 50)),
        (jobs[2].clone(), estimate(1, 10, 10)),
    ]);
    let plan = CampaignWavePlanner::try_plan(snapshot.policy(), &requests).unwrap();
    assert_eq!(plan.work(), &[jobs[0].clone(), jobs[1].clone()]);

    let subset = BTreeMap::from([(jobs[0].clone(), estimate(1, 100, 50))]);
    let mismatched = CampaignWavePlanner::try_plan(snapshot.policy(), &subset).unwrap();
    let before = controller.try_usage().unwrap();
    assert!(matches!(
        controller.try_reserve_wave(&snapshot, &mismatched, &requests),
        Err(CampaignAdmissionError::WavePlanMismatch)
    ));
    assert_eq!(controller.try_usage().unwrap(), before);

    let mut tasks = controller
        .try_reserve_wave(&snapshot, &plan, &requests)
        .unwrap()
        .into_tasks();
    let charged = controller.try_usage().unwrap();
    assert_eq!(charged.in_flight_cores(), 2);
    assert_eq!(
        charged.in_flight_peak_additional_memory(),
        CampaignBytes::new(300)
    );
    assert_eq!(charged.total_charged_memory(), CampaignBytes::new(400));
    assert!(matches!(
        controller.try_snapshot(),
        Err(CampaignAdmissionError::WaveStillInFlight { .. })
    ));
    assert!(matches!(
        controller.try_reserve_wave(&snapshot, &plan, &requests),
        Err(CampaignAdmissionError::WaveStillInFlight { .. })
    ));

    drop(tasks.pop().unwrap());
    let half = controller.try_usage().unwrap();
    assert_eq!(half.in_flight_cores(), 1);
    assert_eq!(
        half.in_flight_peak_additional_memory(),
        CampaignBytes::new(150)
    );
    drop(tasks);
    let quiescent = controller.try_usage().unwrap();
    assert_eq!(quiescent.in_flight_cores(), 0);
    assert_eq!(quiescent.total_charged_memory(), CampaignBytes::new(100));
    assert!(matches!(
        controller.try_reserve_wave(&snapshot, &plan, &requests),
        Err(CampaignAdmissionError::StaleSnapshot { .. })
    ));
}

#[test]
fn same_job_exceptional_leaf_units_keep_distinct_reservations_and_resident_tokens() {
    let planned_job = jobs(1).remove(0).job().clone();
    let left = CampaignWorkKey::exact_publication_exceptional_leaf(
        planned_job.clone(),
        "same-context",
        8,
        2,
        13,
        4,
    );
    let right = CampaignWorkKey::exact_publication_exceptional_leaf(
        planned_job.clone(),
        "same-context",
        8,
        2,
        13,
        5,
    );
    let mut controller = controller(2, 1_000, 100);
    let snapshot = controller.try_snapshot().unwrap();
    let requests = BTreeMap::from([
        (right.clone(), estimate(1, 100, 50)),
        (left.clone(), estimate(1, 100, 50)),
    ]);
    let plan = CampaignWavePlanner::try_plan(snapshot.policy(), &requests).unwrap();
    assert_eq!(plan.work(), &[left.clone(), right.clone()]);

    let wave = controller
        .try_reserve_wave(&snapshot, &plan, &requests)
        .unwrap();
    assert_eq!(
        wave.tasks()
            .iter()
            .map(rustred::CampaignTaskReservation::work)
            .collect::<Vec<_>>(),
        vec![&left, &right]
    );
    let mut residents = wave
        .into_tasks()
        .into_iter()
        .enumerate()
        .map(|(ordinal, reservation)| reservation.bind(ordinal).try_commit_initial().unwrap())
        .collect::<Vec<_>>();

    assert_eq!(residents[0].token().work(), &left);
    assert_eq!(residents[1].token().work(), &right);
    assert_eq!(residents[0].token().job(), &planned_job);
    assert_eq!(residents[1].token().job(), &planned_job);
    assert_ne!(
        residents[0].token().generation(),
        residents[1].token().generation()
    );

    let snapshot = controller.try_snapshot().unwrap();
    let retry_requests = one_request(&right, 100, 50);
    let retry_plan = CampaignWavePlanner::try_plan(snapshot.policy(), &retry_requests).unwrap();
    let wrong_predecessor = BTreeMap::from([(right.clone(), residents[0].token().clone())]);
    let before_rejection = controller.try_usage().unwrap();
    assert!(matches!(
        controller.try_reserve_wave_with_predecessors(
            &snapshot,
            &retry_plan,
            &retry_requests,
            &wrong_predecessor,
        ),
        Err(CampaignAdmissionError::ResidentWorkMismatch { expected, actual })
            if expected == right && actual == left
    ));
    assert_eq!(controller.try_usage().unwrap(), before_rejection);

    residents.clear();
    assert_eq!(
        controller
            .try_usage()
            .unwrap()
            .baseline()
            .hydrated_retained(),
        CampaignBytes::ZERO
    );
}

#[test]
fn concurrent_normal_and_panic_drops_release_each_charge_exactly_once() {
    let jobs = jobs(4);
    let mut controller = controller(4, 1_000, 10);
    let snapshot = controller.try_snapshot().unwrap();
    let requests = jobs
        .iter()
        .cloned()
        .map(|job| (job, estimate(1, 10, 20)))
        .collect::<BTreeMap<_, _>>();
    let plan = CampaignWavePlanner::try_plan(snapshot.policy(), &requests).unwrap();
    let tasks = controller
        .try_reserve_wave(&snapshot, &plan, &requests)
        .unwrap()
        .into_tasks();
    assert_eq!(tasks.len(), 4);

    let ready = Arc::new(Barrier::new(tasks.len() + 1));
    let release = Arc::new(Barrier::new(tasks.len() + 1));
    let drops = Arc::new(AtomicUsize::new(0));
    let panics = std::thread::scope(|scope| {
        let handles = tasks
            .into_iter()
            .enumerate()
            .map(|(ordinal, task)| {
                let ready = Arc::clone(&ready);
                let release = Arc::clone(&release);
                let drops = Arc::clone(&drops);
                scope.spawn(move || {
                    catch_unwind(AssertUnwindSafe(|| {
                        let admitted = task.bind(CountedOwner {
                            id: "concurrent",
                            drops,
                        });
                        ready.wait();
                        release.wait();
                        if ordinal % 2 == 0 {
                            panic!("injected worker panic {ordinal}");
                        }
                        drop(admitted);
                    }))
                    .is_err()
                })
            })
            .collect::<Vec<_>>();

        ready.wait();
        let charged = controller.try_usage().unwrap();
        assert_eq!(charged.in_flight_cores(), 4);
        assert_eq!(
            charged.in_flight_peak_additional_memory(),
            CampaignBytes::new(120)
        );
        release.wait();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>()
    });

    assert_eq!(panics, vec![true, false, true, false]);
    assert_eq!(drops.load(Ordering::SeqCst), 4);
    let usage = controller.try_usage().unwrap();
    assert_eq!(usage.in_flight_cores(), 0);
    assert_eq!(
        usage.in_flight_peak_additional_memory(),
        CampaignBytes::ZERO
    );
    assert_eq!(usage.total_charged_memory(), CampaignBytes::new(10));
}

#[test]
fn retained_replacement_charges_old_and_new_until_old_owner_finishes_drop() {
    let job = jobs(1).pop().unwrap();
    let mut controller = controller(1, 1_000, 100);
    let (entered_tx, entered_rx) = mpsc::sync_channel(0);
    let (release_tx, release_rx) = mpsc::sync_channel(0);
    let old_drops = Arc::new(AtomicUsize::new(0));
    let old = commit_initial(
        &mut controller,
        &job,
        400,
        BlockingOwner {
            entered: entered_tx,
            release: release_rx,
            drops: Arc::clone(&old_drops),
        },
    );
    assert_eq!(
        controller.try_usage().unwrap().total_charged_memory(),
        CampaignBytes::new(500)
    );

    let predecessor = old.token().clone();
    let task = reserve_one(&mut controller, &job, 450, 50, Some(predecessor));
    let successor_drops = Arc::new(AtomicUsize::new(0));
    let admitted = task.bind(CountedOwner {
        id: "successor",
        drops: Arc::clone(&successor_drops),
    });

    let successor = std::thread::scope(|scope| {
        let handle = scope.spawn(move || admitted.try_commit_successor(Some(old)));
        entered_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("old resident destructor did not reach its blocking boundary");

        let overlap = controller.try_usage().unwrap();
        assert_eq!(overlap.in_flight_cores(), 1);
        assert_eq!(
            overlap.in_flight_peak_additional_memory(),
            CampaignBytes::new(50)
        );
        assert_eq!(
            overlap.baseline().hydrated_retained(),
            CampaignBytes::new(850)
        );
        assert_eq!(overlap.total_charged_memory(), CampaignBytes::new(1_000));
        assert_eq!(old_drops.load(Ordering::SeqCst), 0);
        assert!(matches!(
            controller.try_snapshot(),
            Err(CampaignAdmissionError::WaveStillInFlight { .. })
        ));

        release_tx.send(()).unwrap();
        handle.join().unwrap().unwrap()
    });

    assert_eq!(old_drops.load(Ordering::SeqCst), 1);
    let committed = controller.try_usage().unwrap();
    assert_eq!(committed.in_flight_cores(), 0);
    assert_eq!(
        committed.baseline().hydrated_retained(),
        CampaignBytes::new(450)
    );
    assert_eq!(committed.total_charged_memory(), CampaignBytes::new(550));
    assert_eq!(successor.retained_output().id, "successor");
    drop(successor);
    assert_eq!(successor_drops.load(Ordering::SeqCst), 1);
    assert_eq!(
        controller.try_usage().unwrap().total_charged_memory(),
        CampaignBytes::new(100)
    );
}

#[test]
fn stale_successor_failure_returns_the_exact_output_and_current_predecessor() {
    let job = jobs(1).pop().unwrap();
    let mut controller = controller(1, 2_000, 100);
    let drops_a = Arc::new(AtomicUsize::new(0));
    let resident_a = commit_initial(
        &mut controller,
        &job,
        100,
        CountedOwner {
            id: "a",
            drops: Arc::clone(&drops_a),
        },
    );
    let stale = resident_a.token().clone();

    let drops_b = Arc::new(AtomicUsize::new(0));
    let resident_b = reserve_one(&mut controller, &job, 120, 10, Some(stale.clone()))
        .bind(CountedOwner {
            id: "b",
            drops: Arc::clone(&drops_b),
        })
        .try_commit_successor(Some(resident_a))
        .unwrap();
    assert_eq!(drops_a.load(Ordering::SeqCst), 1);
    assert_ne!(resident_b.token().generation(), stale.generation());

    let drops_c = Arc::new(AtomicUsize::new(0));
    let admitted_c =
        reserve_one(&mut controller, &job, 140, 20, Some(stale.clone())).bind(CountedOwner {
            id: "c",
            drops: Arc::clone(&drops_c),
        });
    let before_failure = controller.try_usage().unwrap();
    let failure = admitted_c
        .try_commit_successor(Some(resident_b))
        .unwrap_err();
    assert!(matches!(
        failure.error(),
        CampaignAdmissionError::ResidentGenerationMismatch {
            expected,
            actual: Some(actual),
            ..
        } if *expected == stale.generation() && *actual != *expected
    ));
    assert_eq!(controller.try_usage().unwrap(), before_failure);

    let (_error, admitted_c, returned_b) = failure.into_parts();
    assert_eq!(admitted_c.retained_output().id, "c");
    let returned_b = returned_b.expect("failed commit must return the current predecessor");
    assert_eq!(returned_b.retained_output().id, "b");
    assert_eq!(drops_b.load(Ordering::SeqCst), 0);
    assert_eq!(drops_c.load(Ordering::SeqCst), 0);

    drop(admitted_c);
    let only_b = controller.try_usage().unwrap();
    assert_eq!(only_b.in_flight_cores(), 0);
    assert_eq!(
        only_b.baseline().hydrated_retained(),
        CampaignBytes::new(120)
    );
    assert_eq!(drops_c.load(Ordering::SeqCst), 1);
    drop(returned_b);
    assert_eq!(drops_b.load(Ordering::SeqCst), 1);
    assert_eq!(
        controller.try_usage().unwrap().total_charged_memory(),
        CampaignBytes::new(100)
    );
}

#[test]
fn snapshots_and_resident_tokens_are_controller_local() {
    let job = jobs(1).pop().unwrap();
    let mut left = controller(1, 1_000, 100);
    let mut right = controller(1, 1_000, 100);
    let left_snapshot = left.try_snapshot().unwrap();
    let requests = one_request(&job, 100, 20);
    let left_plan = CampaignWavePlanner::try_plan(left_snapshot.policy(), &requests).unwrap();
    let right_before = right.try_usage().unwrap();
    assert!(matches!(
        right.try_reserve_wave(&left_snapshot, &left_plan, &requests),
        Err(CampaignAdmissionError::ForeignSnapshot)
    ));
    assert_eq!(right.try_usage().unwrap(), right_before);

    let left_resident = commit_initial(
        &mut left,
        &job,
        100,
        CountedOwner {
            id: "left",
            drops: Arc::new(AtomicUsize::new(0)),
        },
    );
    let right_snapshot = right.try_snapshot().unwrap();
    let right_plan = CampaignWavePlanner::try_plan(right_snapshot.policy(), &requests).unwrap();
    let foreign_predecessor = BTreeMap::from([(job.clone(), left_resident.token().clone())]);
    assert!(matches!(
        right.try_reserve_wave_with_predecessors(
            &right_snapshot,
            &right_plan,
            &requests,
            &foreign_predecessor,
        ),
        Err(CampaignAdmissionError::ForeignResidentToken { .. })
    ));
    assert_eq!(right.try_usage().unwrap(), right_before);
    drop(left_resident);
}

#[test]
fn construction_and_owner_drop_panics_release_the_complete_task_charge() {
    let job = jobs(1).pop().unwrap();
    let mut controller = controller(1, 1_000, 100);

    let construction_task = reserve_one(&mut controller, &job, 60, 40, None);
    assert_eq!(
        controller.try_usage().unwrap().total_charged_memory(),
        CampaignBytes::new(200)
    );
    let construction_panic = catch_unwind(AssertUnwindSafe(|| {
        let _: Result<rustred::CampaignAdmittedTask<CountedOwner>, ()> = construction_task
            .try_build(|_| -> Result<CountedOwner, ()> {
                panic!("injected successor-construction panic")
            });
    }));
    assert!(construction_panic.is_err());
    let after_construction = controller.try_usage().unwrap();
    assert_eq!(after_construction.in_flight_cores(), 0);
    assert_eq!(
        after_construction.total_charged_memory(),
        CampaignBytes::new(100)
    );

    let destructor_drops = Arc::new(AtomicUsize::new(0));
    let destructor_task = reserve_one(&mut controller, &job, 60, 40, None);
    let admitted = destructor_task.bind(PanickingOwner {
        drops: Arc::clone(&destructor_drops),
    });
    let destructor_panic = catch_unwind(AssertUnwindSafe(|| drop(admitted)));
    assert!(destructor_panic.is_err());
    assert_eq!(destructor_drops.load(Ordering::SeqCst), 1);
    let after_destructor = controller.try_usage().unwrap();
    assert_eq!(after_destructor.in_flight_cores(), 0);
    assert_eq!(
        after_destructor.in_flight_peak_additional_memory(),
        CampaignBytes::ZERO
    );
    assert_eq!(
        after_destructor.total_charged_memory(),
        CampaignBytes::new(100)
    );
}

#[test]
fn predecessor_drop_panic_unwinds_old_new_transient_and_core_charges() {
    let job = jobs(1).pop().unwrap();
    let mut controller = controller(1, 600, 100);
    let predecessor_drops = Arc::new(AtomicUsize::new(0));
    let predecessor = commit_initial(
        &mut controller,
        &job,
        200,
        PanickingOwner {
            drops: Arc::clone(&predecessor_drops),
        },
    );
    let predecessor_token = predecessor.token().clone();
    let successor_drops = Arc::new(AtomicUsize::new(0));
    let admitted =
        reserve_one(&mut controller, &job, 250, 50, Some(predecessor_token)).bind(CountedOwner {
            id: "successor-during-unwind",
            drops: Arc::clone(&successor_drops),
        });
    let peak = controller.try_usage().unwrap();
    assert_eq!(peak.in_flight_cores(), 1);
    assert_eq!(peak.total_charged_memory(), CampaignBytes::new(600));

    let panic = catch_unwind(AssertUnwindSafe(|| {
        drop(admitted.try_commit_successor(Some(predecessor)));
    }));
    assert!(panic.is_err());
    assert_eq!(predecessor_drops.load(Ordering::SeqCst), 1);
    assert_eq!(successor_drops.load(Ordering::SeqCst), 1);
    let after = controller.try_usage().unwrap();
    assert_eq!(after.in_flight_cores(), 0);
    assert_eq!(after.baseline().hydrated_retained(), CampaignBytes::ZERO);
    assert_eq!(after.total_charged_memory(), CampaignBytes::new(100));
}

#[test]
fn empty_waves_are_noops_and_unselected_predecessors_are_rejected() {
    fn assert_send<T: Send>() {}
    assert_send::<rustred::CampaignTaskReservation>();

    let jobs = jobs(2);
    let mut controller = controller(1, 1_000, 100);
    let empty_requests = BTreeMap::new();
    let empty_snapshot = controller.try_snapshot().unwrap();
    let empty_plan =
        CampaignWavePlanner::try_plan(empty_snapshot.policy(), &empty_requests).unwrap();
    let before_empty = controller.try_usage().unwrap();
    for _ in 0..2 {
        let reservation = controller
            .try_reserve_wave(&empty_snapshot, &empty_plan, &empty_requests)
            .unwrap();
        assert!(reservation.is_empty());
        assert_eq!(controller.try_usage().unwrap(), before_empty);
    }

    let resident = commit_initial(&mut controller, &jobs[1], 100, "second-job");
    let snapshot = controller.try_snapshot().unwrap();
    let requests = one_request(&jobs[0], 100, 20);
    let plan = CampaignWavePlanner::try_plan(snapshot.policy(), &requests).unwrap();
    let predecessors = BTreeMap::from([(jobs[1].clone(), resident.token().clone())]);
    let before_rejection = controller.try_usage().unwrap();
    assert!(matches!(
        controller.try_reserve_wave_with_predecessors(
            &snapshot,
            &plan,
            &requests,
            &predecessors,
        ),
        Err(CampaignAdmissionError::UnexpectedPredecessorToken { work }) if work == jobs[1]
    ));
    assert_eq!(controller.try_usage().unwrap(), before_rejection);
    drop(resident);
}

#[test]
fn admitted_executor_moves_guards_to_owned_workers_and_returns_stable_outcomes() {
    let n_cores = std::thread::available_parallelism().unwrap().get().min(4);
    if n_cores < 3 || !symbolica::LicenseManager::is_licensed() {
        return;
    }
    let jobs = jobs(n_cores);
    let mut controller = controller(n_cores, 10_000, 10);
    let snapshot = controller.try_snapshot().unwrap();
    let requests = jobs
        .iter()
        .cloned()
        .map(|job| (job, estimate(1, 10, 20)))
        .collect::<BTreeMap<_, _>>();
    let plan = CampaignWavePlanner::try_plan(snapshot.policy(), &requests).unwrap();
    let wave = controller
        .try_reserve_wave(&snapshot, &plan, &requests)
        .unwrap();
    let ordinals = jobs
        .iter()
        .cloned()
        .enumerate()
        .map(|(ordinal, job)| (job, ordinal))
        .collect::<BTreeMap<_, _>>();
    let barrier = Arc::new(Barrier::new(n_cores));
    let drops = Arc::new(AtomicUsize::new(0));

    let outcomes = controller
        .execute_reserved_wave(wave, {
            let barrier = Arc::clone(&barrier);
            let drops = Arc::clone(&drops);
            move |task| -> Result<CountedOwner, CountedOwner> {
                let ordinal = ordinals[task.work()];
                barrier.wait();
                match ordinal {
                    1 => Err(CountedOwner {
                        id: "typed-failure",
                        drops: Arc::clone(&drops),
                    }),
                    2 => panic_any(CountedOwner {
                        id: "panic-payload",
                        drops: Arc::clone(&drops),
                    }),
                    _ => Ok(CountedOwner {
                        id: "built",
                        drops: Arc::clone(&drops),
                    }),
                }
            }
        })
        .unwrap();

    assert_eq!(
        outcomes
            .iter()
            .map(CampaignTaskExecution::work)
            .collect::<Vec<_>>(),
        jobs.iter().collect::<Vec<_>>()
    );
    assert!(matches!(outcomes[0], CampaignTaskExecution::Built(_)));
    assert!(matches!(outcomes[1], CampaignTaskExecution::Failed(_)));
    assert!(matches!(outcomes[2], CampaignTaskExecution::Panicked(_)));
    let CampaignTaskExecution::Failed(failure) = &outcomes[1] else {
        unreachable!()
    };
    assert_eq!(failure.error().id, "typed-failure");
    let CampaignTaskExecution::Panicked(panic) = &outcomes[2] else {
        unreachable!()
    };
    assert_eq!(panic.message(), None);

    let charged = controller.try_usage().unwrap();
    assert_eq!(charged.in_flight_cores(), n_cores);
    assert_eq!(
        charged.in_flight_peak_additional_memory(),
        CampaignBytes::new((n_cores as u64) * 30)
    );
    assert_eq!(
        charged.total_charged_memory(),
        CampaignBytes::new(10 + (n_cores as u64) * 30)
    );
    assert_eq!(drops.load(Ordering::SeqCst), 0);

    drop(outcomes);
    assert_eq!(drops.load(Ordering::SeqCst), n_cores);
    let released = controller.try_usage().unwrap();
    assert_eq!(released.in_flight_cores(), 0);
    assert_eq!(
        released.in_flight_peak_additional_memory(),
        CampaignBytes::ZERO
    );
    assert_eq!(released.total_charged_memory(), CampaignBytes::new(10));
}

#[test]
fn admitted_executor_rejects_a_foreign_wave_before_any_callback() {
    let job = jobs(1).pop().unwrap();
    let mut owner = controller(1, 1_000, 100);
    let mut foreign = controller(1, 1_000, 100);
    let snapshot = owner.try_snapshot().unwrap();
    let requests = one_request(&job, 60, 40);
    let plan = CampaignWavePlanner::try_plan(snapshot.policy(), &requests).unwrap();
    let wave = owner.try_reserve_wave(&snapshot, &plan, &requests).unwrap();
    let callbacks = AtomicUsize::new(0);

    let failure = match foreign.execute_reserved_wave(wave, |_| -> Result<(), ()> {
        callbacks.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }) {
        Err(failure) => failure,
        Ok(_) => panic!("a foreign controller must not execute an admitted wave"),
    };
    assert!(matches!(
        failure.error(),
        CampaignAdmissionError::ForeignWaveReservation
    ));
    assert_eq!(callbacks.load(Ordering::SeqCst), 0);
    assert_eq!(
        owner.try_usage().unwrap().total_charged_memory(),
        CampaignBytes::new(200)
    );
    assert_eq!(
        foreign.try_usage().unwrap().total_charged_memory(),
        CampaignBytes::new(100)
    );
    let (_, wave) = failure.into_parts();
    drop(wave);
    assert_eq!(
        owner.try_usage().unwrap().total_charged_memory(),
        CampaignBytes::new(100)
    );
}

#[test]
fn admitted_executor_rejects_weighted_tasks_until_inner_parallelism_is_controlled() {
    let job = jobs(1).pop().unwrap();
    let mut controller = controller(2, 1_000, 100);
    let snapshot = controller.try_snapshot().unwrap();
    let requests = BTreeMap::from([(job.clone(), estimate(2, 60, 40))]);
    let plan = CampaignWavePlanner::try_plan(snapshot.policy(), &requests).unwrap();
    let wave = controller
        .try_reserve_wave(&snapshot, &plan, &requests)
        .unwrap();
    let callbacks = AtomicUsize::new(0);

    let failure = match controller.execute_reserved_wave(wave, |_| -> Result<(), ()> {
        callbacks.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }) {
        Err(failure) => failure,
        Ok(_) => panic!("weighted tasks require a controlled inner-pool adapter"),
    };
    assert!(matches!(
        failure.error(),
        CampaignAdmissionError::UnsupportedExecutorCoreWidth {
            work: rejected,
            requested: 2,
        } if rejected == &job
    ));
    assert_eq!(callbacks.load(Ordering::SeqCst), 0);
    let (_, wave) = failure.into_parts();
    drop(wave);
    assert_eq!(
        controller.try_usage().unwrap().total_charged_memory(),
        CampaignBytes::new(100)
    );
}

#[test]
fn executor_failure_destructor_panic_still_releases_its_reservation() {
    let job = jobs(1).pop().unwrap();
    let mut controller = controller(1, 1_000, 100);
    let snapshot = controller.try_snapshot().unwrap();
    let requests = one_request(&job, 60, 40);
    let plan = CampaignWavePlanner::try_plan(snapshot.policy(), &requests).unwrap();
    let wave = controller
        .try_reserve_wave(&snapshot, &plan, &requests)
        .unwrap();
    let drops = Arc::new(AtomicUsize::new(0));
    let outcomes = controller
        .execute_reserved_wave(wave, {
            let drops = Arc::clone(&drops);
            move |_| -> Result<(), PanickingOwner> {
                Err(PanickingOwner {
                    drops: Arc::clone(&drops),
                })
            }
        })
        .unwrap();
    assert_eq!(
        controller.try_usage().unwrap().total_charged_memory(),
        CampaignBytes::new(200)
    );

    let panic = catch_unwind(AssertUnwindSafe(|| drop(outcomes)));
    assert!(panic.is_err());
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    let released = controller.try_usage().unwrap();
    assert_eq!(released.in_flight_cores(), 0);
    assert_eq!(released.total_charged_memory(), CampaignBytes::new(100));
}

#[test]
fn resident_transform_keeps_old_and_new_charged_until_successor_commit() {
    #[derive(Debug)]
    struct TransformOwner {
        value: usize,
        drops: Arc<AtomicUsize>,
    }

    impl Drop for TransformOwner {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::SeqCst);
        }
    }

    let job = jobs(1).pop().unwrap();
    let mut controller = controller(1, 600, 100);
    let drops = Arc::new(AtomicUsize::new(0));
    let predecessor = commit_initial(
        &mut controller,
        &job,
        200,
        TransformOwner {
            value: 7,
            drops: Arc::clone(&drops),
        },
    );
    let token = predecessor.token().clone();
    let task = reserve_one(&mut controller, &job, 250, 50, Some(token))
        .try_bind_resident_transform(predecessor)
        .unwrap();
    assert_eq!(
        controller.try_usage().unwrap().total_charged_memory(),
        CampaignBytes::new(600)
    );

    let mut outcomes = controller
        .execute_resident_transforms_ordered(
            vec![task],
            |context,
             mut owner|
             -> Result<
                TransformOwner,
                CampaignResidentTransformBuildFailure<TransformOwner, ()>,
            > {
                assert_eq!(context.cores(), 1);
                owner.value += 1;
                Ok(owner)
            },
        )
        .unwrap();
    let CampaignResidentTransformExecution::Committed(successor) = outcomes.pop().unwrap() else {
        panic!("resident transform must commit")
    };
    assert_eq!(successor.retained_output().value, 8);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    let committed = controller.try_usage().unwrap();
    assert_eq!(committed.in_flight_cores(), 0);
    assert_eq!(
        committed.baseline().hydrated_retained(),
        CampaignBytes::new(250)
    );
    assert_eq!(committed.total_charged_memory(), CampaignBytes::new(350));

    drop(successor);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(
        controller.try_usage().unwrap().total_charged_memory(),
        CampaignBytes::new(100)
    );
}

#[test]
fn resident_transform_executor_rejects_foreign_tasks_before_moving_the_owner() {
    let job = jobs(1).pop().unwrap();
    let mut owner = controller(1, 1_000, 100);
    let mut foreign = controller(1, 1_000, 100);
    let drops = Arc::new(AtomicUsize::new(0));
    let predecessor = commit_initial(
        &mut owner,
        &job,
        200,
        CountedOwner {
            id: "foreign-predecessor",
            drops: Arc::clone(&drops),
        },
    );
    let token = predecessor.token().clone();
    let task = reserve_one(&mut owner, &job, 250, 50, Some(token))
        .try_bind_resident_transform(predecessor)
        .unwrap();
    let callbacks = AtomicUsize::new(0);

    let failure = match foreign.execute_resident_transforms_ordered(
        vec![task],
        |_,
         owner|
         -> Result<CountedOwner, CampaignResidentTransformBuildFailure<CountedOwner, ()>> {
            callbacks.fetch_add(1, Ordering::SeqCst);
            Ok(owner)
        },
    ) {
        Err(failure) => failure,
        Ok(_) => panic!("a foreign controller must not execute resident transforms"),
    };
    assert!(matches!(
        failure.error(),
        CampaignAdmissionError::ForeignTaskReservation { work: rejected } if rejected == &job
    ));
    assert_eq!(callbacks.load(Ordering::SeqCst), 0);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert_eq!(
        owner.try_usage().unwrap().total_charged_memory(),
        CampaignBytes::new(600)
    );
    let (_, tasks) = failure.into_parts();
    drop(tasks);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(
        owner.try_usage().unwrap().total_charged_memory(),
        CampaignBytes::new(100)
    );
    assert_eq!(
        foreign.try_usage().unwrap().total_charged_memory(),
        CampaignBytes::new(100)
    );
}

#[test]
fn resident_transform_executor_canonicalizes_reversed_input_and_completion() {
    let jobs = jobs(3);
    let mut controller = controller(3, 2_000, 100);
    let mut residents = BTreeMap::new();
    for (ordinal, job) in jobs.iter().enumerate() {
        residents.insert(
            job.clone(),
            commit_initial(&mut controller, job, 100, ordinal),
        );
    }

    let snapshot = controller.try_snapshot().unwrap();
    let requests = jobs
        .iter()
        .cloned()
        .map(|job| (job, estimate(1, 120, 30)))
        .collect::<BTreeMap<_, _>>();
    let plan = CampaignWavePlanner::try_plan(snapshot.policy(), &requests).unwrap();
    let predecessors = residents
        .iter()
        .map(|(job, resident)| (job.clone(), resident.token().clone()))
        .collect::<BTreeMap<_, _>>();
    let reservations = controller
        .try_reserve_wave_with_predecessors(&snapshot, &plan, &requests, &predecessors)
        .unwrap()
        .into_tasks();
    let mut transforms = reservations
        .into_iter()
        .map(|reservation| {
            let job = reservation.work().clone();
            reservation
                .try_bind_resident_transform(residents.remove(&job).unwrap())
                .unwrap()
        })
        .collect::<Vec<_>>();
    transforms.reverse();
    let worker_barrier = Arc::new(Barrier::new(jobs.len()));
    let next_completion = Arc::new(AtomicUsize::new(jobs.len() - 1));
    let completion_order = Arc::new(std::sync::Mutex::new(Vec::new()));

    let outcomes = controller
        .execute_resident_transforms_ordered(transforms, {
            let worker_barrier = Arc::clone(&worker_barrier);
            let next_completion = Arc::clone(&next_completion);
            let completion_order = Arc::clone(&completion_order);
            move |_, ordinal| -> Result<usize, CampaignResidentTransformBuildFailure<usize, ()>> {
                worker_barrier.wait();
                while next_completion.load(Ordering::SeqCst) != ordinal {
                    std::thread::yield_now();
                }
                completion_order.lock().unwrap().push(ordinal);
                if ordinal > 0 {
                    next_completion.store(ordinal - 1, Ordering::SeqCst);
                }
                Ok(ordinal + 10)
            }
        })
        .unwrap();
    assert_eq!(*completion_order.lock().unwrap(), vec![2, 1, 0]);
    assert_eq!(
        outcomes
            .iter()
            .map(CampaignResidentTransformExecution::work)
            .collect::<Vec<_>>(),
        jobs.iter().collect::<Vec<_>>()
    );
    for (ordinal, outcome) in outcomes.iter().enumerate() {
        let CampaignResidentTransformExecution::Committed(resident) = outcome else {
            panic!("every canonical transform must commit")
        };
        assert_eq!(*resident.retained_output(), ordinal + 10);
    }
    drop(outcomes);
    assert_eq!(
        controller.try_usage().unwrap().total_charged_memory(),
        CampaignBytes::new(100)
    );
}

#[test]
fn resident_transform_typed_failure_recovers_the_exact_predecessor() {
    let job = jobs(1).pop().unwrap();
    let mut controller = controller(1, 600, 100);
    let predecessor_drops = Arc::new(AtomicUsize::new(0));
    let error_drops = Arc::new(AtomicUsize::new(0));
    let predecessor = commit_initial(
        &mut controller,
        &job,
        200,
        CountedOwner {
            id: "predecessor",
            drops: Arc::clone(&predecessor_drops),
        },
    );
    let token = predecessor.token().clone();
    let task = reserve_one(&mut controller, &job, 250, 50, Some(token))
        .try_bind_resident_transform(predecessor)
        .unwrap();
    let mut outcomes = controller
        .execute_resident_transforms_ordered(vec![task], {
            let error_drops = Arc::clone(&error_drops);
            move |_,
                  owner|
                  -> Result<
                CountedOwner,
                CampaignResidentTransformBuildFailure<CountedOwner, CountedOwner>,
            > {
                Err(CampaignResidentTransformBuildFailure::new(
                    owner,
                    CountedOwner {
                        id: "build-error",
                        drops: Arc::clone(&error_drops),
                    },
                ))
            }
        })
        .unwrap();
    let CampaignResidentTransformExecution::BuildFailed(failure) = outcomes.pop().unwrap() else {
        panic!("resident transform must retain its typed failure")
    };
    assert_eq!(failure.error().id, "build-error");
    assert_eq!(predecessor_drops.load(Ordering::SeqCst), 0);
    assert_eq!(error_drops.load(Ordering::SeqCst), 0);
    assert_eq!(
        controller.try_usage().unwrap().total_charged_memory(),
        CampaignBytes::new(600)
    );

    let predecessor = failure.recover_callback_owner();
    assert_eq!(error_drops.load(Ordering::SeqCst), 1);
    assert_eq!(predecessor.retained_output().id, "predecessor");
    assert_eq!(predecessor_drops.load(Ordering::SeqCst), 0);
    assert_eq!(
        controller.try_usage().unwrap().total_charged_memory(),
        CampaignBytes::new(300)
    );
    drop(predecessor);
    assert_eq!(predecessor_drops.load(Ordering::SeqCst), 1);
    assert_eq!(
        controller.try_usage().unwrap().total_charged_memory(),
        CampaignBytes::new(100)
    );
}

#[test]
fn resident_transform_panic_drops_owner_but_keeps_both_charges_until_report_drop() {
    let job = jobs(1).pop().unwrap();
    let mut controller = controller(1, 600, 100);
    let predecessor_drops = Arc::new(AtomicUsize::new(0));
    let predecessor = commit_initial(
        &mut controller,
        &job,
        200,
        CountedOwner {
            id: "panic-predecessor",
            drops: Arc::clone(&predecessor_drops),
        },
    );
    let token = predecessor.token().clone();
    let task = reserve_one(&mut controller, &job, 250, 50, Some(token))
        .try_bind_resident_transform(predecessor)
        .unwrap();
    let mut outcomes =
        controller
            .execute_resident_transforms_ordered(
                vec![task],
                |_,
                 _owner|
                 -> Result<
                    CountedOwner,
                    CampaignResidentTransformBuildFailure<CountedOwner, ()>,
                > { panic!("injected resident transform panic") },
            )
            .unwrap();
    assert_eq!(predecessor_drops.load(Ordering::SeqCst), 1);
    assert_eq!(
        controller.try_usage().unwrap().total_charged_memory(),
        CampaignBytes::new(600)
    );
    let CampaignResidentTransformExecution::Panicked(panic) = outcomes.pop().unwrap() else {
        panic!("resident transform panic must be recovered")
    };
    assert_eq!(panic.message(), Some("injected resident transform panic"));
    drop(panic);
    let released = controller.try_usage().unwrap();
    assert_eq!(released.in_flight_cores(), 0);
    assert_eq!(released.baseline().hydrated_retained(), CampaignBytes::ZERO);
    assert_eq!(released.total_charged_memory(), CampaignBytes::new(100));
}

#[test]
fn resident_transform_error_destructor_panic_releases_owner_and_both_charges() {
    let job = jobs(1).pop().unwrap();
    let mut controller = controller(1, 600, 100);
    let predecessor_drops = Arc::new(AtomicUsize::new(0));
    let error_drops = Arc::new(AtomicUsize::new(0));
    let predecessor = commit_initial(
        &mut controller,
        &job,
        200,
        CountedOwner {
            id: "error-drop-predecessor",
            drops: Arc::clone(&predecessor_drops),
        },
    );
    let token = predecessor.token().clone();
    let task = reserve_one(&mut controller, &job, 250, 50, Some(token))
        .try_bind_resident_transform(predecessor)
        .unwrap();
    let outcomes = controller
        .execute_resident_transforms_ordered(vec![task], {
            let error_drops = Arc::clone(&error_drops);
            move |_,
                  owner|
                  -> Result<
                CountedOwner,
                CampaignResidentTransformBuildFailure<CountedOwner, PanickingOwner>,
            > {
                Err(CampaignResidentTransformBuildFailure::new(
                    owner,
                    PanickingOwner {
                        drops: Arc::clone(&error_drops),
                    },
                ))
            }
        })
        .unwrap();

    let panic = catch_unwind(AssertUnwindSafe(|| drop(outcomes)));
    assert!(panic.is_err());
    assert_eq!(error_drops.load(Ordering::SeqCst), 1);
    assert_eq!(predecessor_drops.load(Ordering::SeqCst), 1);
    assert_eq!(
        controller.try_usage().unwrap().total_charged_memory(),
        CampaignBytes::new(100)
    );
}

#[test]
fn resident_transform_panic_payload_destructor_panic_releases_both_charges() {
    let job = jobs(1).pop().unwrap();
    let mut controller = controller(1, 600, 100);
    let predecessor_drops = Arc::new(AtomicUsize::new(0));
    let payload_drops = Arc::new(AtomicUsize::new(0));
    let predecessor = commit_initial(
        &mut controller,
        &job,
        200,
        CountedOwner {
            id: "panic-payload-predecessor",
            drops: Arc::clone(&predecessor_drops),
        },
    );
    let token = predecessor.token().clone();
    let task = reserve_one(&mut controller, &job, 250, 50, Some(token))
        .try_bind_resident_transform(predecessor)
        .unwrap();
    let outcomes = controller
        .execute_resident_transforms_ordered(vec![task], {
            let payload_drops = Arc::clone(&payload_drops);
            move |_,
                  _owner|
                  -> Result<
                CountedOwner,
                CampaignResidentTransformBuildFailure<CountedOwner, ()>,
            > {
                panic_any(PanickingOwner {
                    drops: Arc::clone(&payload_drops),
                })
            }
        })
        .unwrap();
    assert_eq!(predecessor_drops.load(Ordering::SeqCst), 1);

    let panic = catch_unwind(AssertUnwindSafe(|| drop(outcomes)));
    assert!(panic.is_err());
    assert_eq!(payload_drops.load(Ordering::SeqCst), 1);
    assert_eq!(
        controller.try_usage().unwrap().total_charged_memory(),
        CampaignBytes::new(100)
    );
}

#[test]
fn resident_transform_rejects_weighted_tasks_before_moving_the_owner() {
    let job = jobs(1).pop().unwrap();
    let mut controller = controller(2, 1_000, 100);
    let drops = Arc::new(AtomicUsize::new(0));
    let predecessor = commit_initial(
        &mut controller,
        &job,
        200,
        CountedOwner {
            id: "weighted-predecessor",
            drops: Arc::clone(&drops),
        },
    );
    let predecessor_token = predecessor.token().clone();
    let snapshot = controller.try_snapshot().unwrap();
    let requests = BTreeMap::from([(job.clone(), estimate(2, 250, 50))]);
    let plan = CampaignWavePlanner::try_plan(snapshot.policy(), &requests).unwrap();
    let predecessors = BTreeMap::from([(job.clone(), predecessor_token)]);
    let reservation = controller
        .try_reserve_wave_with_predecessors(&snapshot, &plan, &requests, &predecessors)
        .unwrap()
        .into_tasks()
        .pop()
        .unwrap();
    let task = reservation
        .try_bind_resident_transform(predecessor)
        .unwrap();
    let callbacks = AtomicUsize::new(0);

    let failure = match controller.execute_resident_transforms_ordered(
        vec![task],
        |_,
         owner|
         -> Result<CountedOwner, CampaignResidentTransformBuildFailure<CountedOwner, ()>> {
            callbacks.fetch_add(1, Ordering::SeqCst);
            Ok(owner)
        },
    ) {
        Err(failure) => failure,
        Ok(_) => panic!("weighted resident transforms require a controlled inner adapter"),
    };
    assert!(matches!(
        failure.error(),
        CampaignAdmissionError::UnsupportedExecutorCoreWidth {
            work: rejected,
            requested: 2,
        } if rejected == &job
    ));
    assert_eq!(callbacks.load(Ordering::SeqCst), 0);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    let (_, tasks) = failure.into_parts();
    drop(tasks);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(
        controller.try_usage().unwrap().total_charged_memory(),
        CampaignBytes::new(100)
    );
}
