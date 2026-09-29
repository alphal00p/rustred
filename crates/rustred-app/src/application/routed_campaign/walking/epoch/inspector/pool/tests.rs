use super::*;
use std::sync::atomic::AtomicUsize;

fn work(key: u64) -> Work {
    Work {
        key,
        bytes: vec![key as u8],
    }
}

#[test]
fn cancel_stops_queued_work_and_body_finishes_before_held_worker_join() {
    let gate = (Mutex::new(false), Condvar::new());
    let returned = AtomicBool::new(false);
    let calls = AtomicUsize::new(0);
    let (saved, receipt) = mpsc::channel();
    let (entered, started) = mpsc::channel();
    std::thread::scope(|scope| {
        let observer_gate = &gate;
        let observed_returned = &returned;
        let observer = scope.spawn(move || {
            receipt
                .recv_timeout(Duration::from_secs(5))
                .expect("body reached pre-join save point");
            assert!(!observed_returned.load(Ordering::Acquire));
            *observer_gate.0.lock().unwrap() = true;
            observer_gate.1.notify_all();
        });
        let job = |bytes: &[u8], stop: &AtomicBool| {
            calls.fetch_add(1, Ordering::Relaxed);
            entered.send(()).unwrap();
            let (guard, timed) = gate
                .1
                .wait_timeout_while(gate.0.lock().unwrap(), Duration::from_secs(5), |released| {
                    !*released
                })
                .unwrap();
            assert!(
                !timed.timed_out() && *guard,
                "held worker released only after body save point"
            );
            assert!(stop.load(Ordering::Acquire));
            returned.store(true, Ordering::Release);
            bytes.to_vec()
        };
        with_polling_pool(1, &job, |pool| {
            pool.submit(vec![work(11), work(22), work(33)]).unwrap();
            assert!(matches!(
                pool.poll(Duration::from_secs(5)).unwrap(),
                Poll::Started(11)
            ));
            started
                .recv_timeout(Duration::from_secs(5))
                .expect("job entered the held native stand-in");
            pool.cancel().unwrap();
            let status = pool.snapshot().unwrap();
            assert_eq!(
                status,
                [
                    Status {
                        key: 11,
                        started: true,
                        returned: false
                    },
                    Status {
                        key: 22,
                        started: false,
                        returned: false
                    },
                    Status {
                        key: 33,
                        started: false,
                        returned: false
                    }
                ]
            );
            assert!(pool.submit(vec![work(44)]).is_err());
            saved.send(()).unwrap();
        })
        .unwrap();
        observer.join().unwrap();
    });
    assert_eq!(
        calls.load(Ordering::Relaxed),
        1,
        "queued jobs were never started"
    );
    assert!(returned.load(Ordering::Acquire));
}

#[test]
fn multiple_batches_have_exact_started_result_receipts_and_panic_is_not_lost() {
    let job = |bytes: &[u8], _: &AtomicBool| {
        if bytes == [2] {
            panic!("synthetic job failure");
        }
        bytes.to_vec()
    };
    with_polling_pool(2, &job, |pool| {
        for keys in [[1, 2], [3, 4]] {
            pool.submit(keys.into_iter().map(work).collect()).unwrap();
            assert!(pool.submit(vec![work(9)]).is_err());
            let mut starts = Vec::new();
            let mut results = Vec::new();
            loop {
                match pool.poll(Duration::from_secs(5)).unwrap() {
                    Poll::Started(key) => starts.push(key),
                    Poll::Result { key, bytes } => results.push((key, bytes)),
                    Poll::Waiting => panic!("tiny test jobs did not return"),
                    Poll::Drained => break,
                }
            }
            starts.sort_unstable();
            results.sort_unstable_by_key(|result| result.0);
            assert_eq!(starts, keys);
            assert_eq!(results.len(), 2);
            for (key, bytes) in results {
                assert_eq!(
                    bytes,
                    if key == 2 {
                        Vec::new()
                    } else {
                        vec![key as u8]
                    }
                );
            }
        }
        assert!(pool.submit(vec![work(7), work(7)]).is_err());
    })
    .unwrap();
    assert!(
        with_polling_pool(0, &job, |_| ()).is_err(),
        "responsive W1 must not oversubscribe silently"
    );
}

#[test]
fn cancellation_discards_result_channel_without_waiting_for_polling() {
    let finished = AtomicUsize::new(0);
    let job = |bytes: &[u8], _: &AtomicBool| {
        finished.fetch_add(1, Ordering::Release);
        bytes.to_vec()
    };
    with_polling_pool(1, &job, |pool| {
        pool.submit(vec![work(1)]).unwrap();
        assert!(matches!(
            pool.poll(Duration::from_secs(5)).unwrap(),
            Poll::Started(1)
        ));
        pool.cancel().unwrap(); // May race with the late result; neither path seals anything.
        assert!(pool.poll(Duration::ZERO).is_err());
    })
    .unwrap();
    assert_eq!(finished.load(Ordering::Acquire), 1);
}

#[test]
fn poisoned_queue_is_fatal_even_when_idle_siblings_keep_channel_connected() {
    let queue = Mutex::new(Queue {
        jobs: VecDeque::new(),
        status: vec![Status {
            key: 1,
            started: true,
            returned: false,
        }],
        shutdown: false,
    });
    let ready = Condvar::new();
    let stop = AtomicBool::new(false);
    let (_live_sender, receiver) = mpsc::channel();
    let mut pool = Pool {
        queue: &queue,
        ready: &ready,
        stop: &stop,
        receiver: Some(receiver),
        receipts: vec![1],
        remaining: 1,
        cancelled: false,
    };
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _guard = queue.lock().unwrap();
            panic!("synthetic worker failure outside job frame");
        }))
        .is_err()
    );
    assert!(
        pool.poll(Duration::ZERO)
            .err()
            .unwrap()
            .contains("poisoned (C5)")
    );
    assert!(pool.cancel().is_err());
    assert!(stop.load(Ordering::Acquire));
    assert!(queue.lock().err().unwrap().into_inner().shutdown);
}
