use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, mpsc};
use std::time::Duration;

use rustred::{
    AffineDenominator, CampaignAdmissionController, CampaignAdmissionError, CampaignBytes,
    CampaignEstimatorRevision, CampaignMemoryEstimate, CampaignPlan, CampaignPlanLimits,
    CampaignResident, CampaignResidentToken, CampaignRootSpec, CampaignTaskMemoryEnvelope,
    CampaignTaskResourceEstimate, CampaignWavePlanner, CoefficientContext, IntegralFamily,
    IntegralOrderingPolicy, ParallelExecution, SectorMask,
};

const REVISION: u64 = 1;

fn jobs(count: usize) -> Vec<rustred::CampaignJobKey> {
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
    .cloned()
    .collect()
}

fn controller(cores: usize, max_memory: u64, fixed: u64) -> CampaignAdmissionController {
    CampaignAdmissionController::try_new(
        ParallelExecution::try_new(cores).unwrap(),
        CampaignEstimatorRevision::try_new(REVISION).unwrap(),
        CampaignBytes::new(max_memory),
        CampaignBytes::new(fixed),
        CampaignBytes::ZERO,
    )
    .unwrap()
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
    job: &rustred::CampaignJobKey,
    retained: u64,
    transient: u64,
) -> BTreeMap<rustred::CampaignJobKey, CampaignTaskResourceEstimate> {
    BTreeMap::from([(job.clone(), estimate(1, retained, transient))])
}

fn reserve_one(
    controller: &mut CampaignAdmissionController,
    job: &rustred::CampaignJobKey,
    retained: u64,
    transient: u64,
    predecessor: Option<CampaignResidentToken>,
) -> rustred::CampaignTaskReservation {
    let snapshot = controller.try_snapshot().unwrap();
    let requests = one_request(job, retained, transient);
    let plan = CampaignWavePlanner::try_plan(snapshot.policy(), &requests).unwrap();
    let predecessors = predecessor
        .map(|token| BTreeMap::from([(job.clone(), token)]))
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
    job: &rustred::CampaignJobKey,
    retained: u64,
    owner: T,
) -> CampaignResident<T> {
    reserve_one(controller, job, retained, 0, None)
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
    assert_eq!(plan.jobs(), &[jobs[0].clone(), jobs[1].clone()]);

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
        Err(CampaignAdmissionError::UnexpectedPredecessorToken { job }) if job == jobs[1]
    ));
    assert_eq!(controller.try_usage().unwrap(), before_rejection);
    drop(resident);
}
