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
            assert_eq!(
                pool.activity().unwrap(),
                Activity {
                    queued: 2,
                    computing: 1,
                    occupied: 3,
                    ..Activity::default()
                }
            );
            assert!(pool.retire_all_returned().is_err());
            pool.cancel().unwrap();
            assert_eq!(
                pool.activity().unwrap(),
                Activity {
                    cancelled_queued: 2,
                    computing: 1,
                    occupied: 3,
                    ..Activity::default()
                }
            );
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
            assert_eq!(pool.take_cancelled_status().unwrap(), status);
            assert!(
                pool.activity().is_none(),
                "moved stop inventory is not a live zero"
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
            assert_eq!(
                pool.activity().unwrap(),
                Activity {
                    returned: 2,
                    occupied: 2,
                    ..Activity::default()
                }
            );
            pool.retire_all_returned().unwrap();
            assert_eq!(pool.activity().unwrap(), Activity::default());
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
fn w200_pool_capacity_refill_cancel_and_join_are_bounded_not_a_throughput_test() {
    // Synthetic callbacks only: this exercises 199 actual inspector threads
    // plus the caller/coordinator, not 200 physical-core or licensed CAS work.
    let authorized = AtomicUsize::new(0);
    let calls = AtomicUsize::new(0);
    let inspect = |bytes: &[u8], stop: &AtomicBool| {
        calls.fetch_add(1, Ordering::Relaxed);
        if bytes == [255] {
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            while !stop.load(Ordering::Acquire) {
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        bytes.to_vec()
    };
    with_authorized_pool(
        199,
        &|| {
            authorized.fetch_add(1, Ordering::Relaxed);
            Ok(())
        },
        &inspect,
        |pool| {
            assert_eq!(authorized.load(Ordering::Relaxed), 199);
            let keys: Vec<_> = (0..215).collect();
            pool.submit_rolling(keys.iter().copied().map(work).collect())
                .unwrap();
            let mut starts = Vec::new();
            let mut returned = Vec::new();
            loop {
                match pool.poll(Duration::from_secs(5)).unwrap() {
                    Poll::Started(key) => starts.push(key),
                    Poll::Result { key, bytes } => {
                        assert_eq!(bytes, [key as u8]);
                        returned.push(key);
                    }
                    Poll::Drained => break,
                    Poll::Waiting => panic!("bounded synthetic W200 batch stalled"),
                }
            }
            starts.sort_unstable();
            returned.sort_unstable();
            assert_eq!(starts, keys);
            assert_eq!(returned, keys);
            pool.retire(&keys).unwrap();
            assert_eq!(pool.activity().unwrap(), Activity::default());
            pool.submit_rolling(vec![work(255)]).unwrap();
            assert!(matches!(
                pool.poll(Duration::from_secs(5)).unwrap(),
                Poll::Started(255)
            ));
            pool.cancel().unwrap();
            let status = pool.take_cancelled_status().unwrap();
            assert_eq!(status.len(), 1);
            assert_eq!(status[0].key, 255);
            assert!(status[0].started);
            assert!(pool.submit_rolling(vec![work(256)]).is_err());
        },
    )
    .unwrap();
    assert_eq!(
        calls.load(Ordering::Relaxed),
        216,
        "all workers joined without losing the active callback"
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
        occupied: vec![true],
        generations: vec![1],
        next_generation: 1,
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
        threads: 1,
        cancelled: false,
        result_memory: Arc::new(memory::Counter::default()),
        recycled: Vec::new(),
        logical_limit: MAX_BATCH,
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

#[test]
fn rolling_refill_and_slot_reuse_progress_while_an_older_worker_is_held() {
    let gate = (Mutex::new(false), Condvar::new());
    let held = AtomicBool::new(false);
    let inspect = |bytes: &[u8], _: &AtomicBool| {
        if bytes == [1] {
            held.store(true, Ordering::Release);
            gate.1.notify_all();
            let (open, wait) = gate
                .1
                .wait_timeout_while(gate.0.lock().unwrap(), Duration::from_secs(10), |open| {
                    !*open
                })
                .unwrap();
            assert!(!wait.timed_out() && *open);
        } else {
            let (_guard, wait) = gate
                .1
                .wait_timeout_while(gate.0.lock().unwrap(), Duration::from_secs(10), |_| {
                    !held.load(Ordering::Acquire)
                })
                .unwrap();
            assert!(!wait.timed_out());
        }
        bytes.to_vec()
    };
    with_polling_pool(2, &inspect, |pool| {
        pool.submit_rolling(vec![work(1), work(2)]).unwrap();
        for expected in 2..=20 {
            loop {
                match pool.poll(Duration::from_secs(5)).unwrap() {
                    Poll::Result { key, bytes } => {
                        assert_eq!(key, expected);
                        assert_eq!(bytes, [expected as u8]);
                        break;
                    }
                    Poll::Started(_) => {}
                    _ => panic!("rolling successor stalled behind held job"),
                }
            }
            assert!(held.load(Ordering::Acquire));
            assert_eq!(
                pool.activity().unwrap(),
                Activity {
                    computing: 1,
                    returned: 1,
                    occupied: 2,
                    ..Activity::default()
                }
            );
            assert!(
                pool.submit_rolling(vec![work(expected)]).is_err(),
                "returned but unmerged sequence stays reserved"
            );
            pool.retire(&[expected]).unwrap();
            assert_eq!(
                pool.activity().unwrap(),
                Activity {
                    computing: 1,
                    occupied: 1,
                    ..Activity::default()
                }
            );
            assert!(
                pool.queue.lock().unwrap().status.len() <= 2,
                "retired slots must be reused, never grow with publication count"
            );
            if expected < 20 {
                pool.submit_rolling(vec![work(expected + 1)]).unwrap();
            }
        }
        assert_eq!(
            pool.snapshot()
                .unwrap()
                .iter()
                .map(|s| s.key)
                .collect::<Vec<_>>(),
            [1]
        );
        *gate.0.lock().unwrap() = true;
        gate.1.notify_all();
        loop {
            match pool.poll(Duration::from_secs(5)).unwrap() {
                Poll::Result { key: 1, .. } => {
                    pool.retire(&[1]).unwrap();
                }
                Poll::Started(_) => {}
                Poll::Drained => break,
                _ => panic!("held job did not finish"),
            }
        }
        assert!(pool.snapshot().unwrap().is_empty());
        assert_eq!(pool.activity().unwrap(), Activity::default());
    })
    .unwrap();
}

#[test]
fn one_worker_capability_refusal_wakes_authorized_idle_siblings_without_dispatch() {
    let checks = AtomicUsize::new(0);
    let calls = AtomicUsize::new(0);
    let authorize = || {
        if checks.fetch_add(1, Ordering::Relaxed) == 0 {
            Err("synthetic unavailable worker capability".into())
        } else {
            Ok(())
        }
    };
    let job = |_: &[u8], _: &AtomicBool| {
        calls.fetch_add(1, Ordering::Relaxed);
        Vec::new()
    };
    let result = with_authorized_pool(3, &authorize, &job, |_| {
        panic!("body cannot run without all capabilities")
    });
    assert!(matches!(result, Err(RunError::Capability(_))));
    assert_eq!(checks.load(Ordering::Relaxed), 3);
    assert_eq!(calls.load(Ordering::Relaxed), 0);
}
mod escrow;
mod snapshot;
