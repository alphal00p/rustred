use super::*;
use crate::application::routed_campaign::walking::inspection::NativeStats;

fn domain() -> Arc<Domain<1>> {
    Arc::new(Domain {
        phase: Phase::Apply,
        owner: [true],
        lower: vec![0],
        upper: vec![None],
        rank: Some(0),
        powers: Default::default(),
    })
}
fn finished() -> Finished {
    Finished {
        stats: NativeStats::Apply(Default::default()),
        error: None,
        error_kind: "none",
        seconds: 0.0,
    }
}

#[test]
fn ready_stream_wait_checks_all_target_tickets_and_ignores_unrelated_finished() {
    let pool = Pool::<1>::new(2);
    assert!(pool.dispatch(0, domain()));
    assert!(pool.dispatch(1, domain()));
    pool.finish(1, finished());
    assert!(pool.wait_for_any_stream(&[0, 1]));
    assert!(!pool.wait_for_any_stream(&[0]));
    assert!(!pool.wait_for_any_stream(&[]));
    let Poll::Finished(_) = pool.poll(1) else {
        panic!("waiting must not consume completion");
    };
    assert!(!pool.wait_for_any_stream(&[0, 1]));
}

#[test]
fn ready_stream_wait_handles_notification_race_failure_and_shutdown() {
    let pool = Pool::<1>::new(1);
    assert!(pool.dispatch(0, domain()));
    std::thread::scope(|scope| {
        // Whether finish wins or loses the mutex race, the same predicate must
        // observe readiness. No signal is required before entering the wait.
        let waiter = scope.spawn(|| {
            let deadline = Instant::now() + Duration::from_secs(10);
            while !pool.wait_for_any_stream(&[0]) {
                assert!(
                    Instant::now() < deadline,
                    "ready notification was never observed"
                );
            }
        });
        pool.finish(0, finished());
        waiter.join().unwrap();
    });
    pool.fail(Failure {
        id: None,
        phase: None,
        kind: "test",
        detail: "injected failure".into(),
    });
    assert!(!pool.wait_for_any_stream(&[0]));
    let pool = Pool::<1>::new(1);
    pool.shutdown();
    assert!(!pool.wait_for_any_stream(&[0]));
}
