use super::*;
use std::cell::RefCell;
use std::collections::HashSet;
use std::sync::atomic::AtomicUsize;
use std::sync::{Condvar, Mutex};
use std::time::Duration;

#[test]
fn ordered_batches_preserve_results_and_borrowed_coordinator_observer() {
    let coordinator = std::thread::current().id();
    for workers in [1, 2, 4] {
        if !crate::test_gates::workers_or_skip("route preparation workers", workers) {
            continue;
        }
        let operation_threads = Mutex::new(HashSet::new());
        // RefCell is deliberately !Sync: progress never crosses the pool.
        let progress = RefCell::new(Vec::new());
        let result = map_batches(
            19,
            workers,
            &AtomicBool::new(false),
            |ordinal| {
                operation_threads
                    .lock()
                    .unwrap()
                    .insert(std::thread::current().id());
                Ok((ordinal % 3 != 0).then_some(ordinal))
            },
            |completed, verified| {
                assert_eq!(std::thread::current().id(), coordinator);
                progress.borrow_mut().push((completed, verified));
            },
        )
        .unwrap()
        .unwrap();
        let expected: Vec<_> = (0..19).filter(|ordinal| ordinal % 3 != 0).collect();
        assert_eq!(result, expected);
        assert_eq!(progress.borrow().last(), Some(&(19, expected.len())));
        let operation_threads = operation_threads.into_inner().unwrap();
        assert!(!operation_threads.is_empty());
        assert!(operation_threads.len() <= workers);
        if workers == 1 {
            assert_eq!(operation_threads, HashSet::from([coordinator]));
        } else {
            assert!(!operation_threads.contains(&coordinator));
        }
    }
}

#[test]
fn errors_follow_input_order_even_when_later_route_finishes_first() {
    if !crate::test_gates::workers_or_skip("ordered route preparation errors", 2) {
        return;
    }
    let finished = (Mutex::new(false), Condvar::new());
    let completion_order = Mutex::new(Vec::new());
    let result = map_batches::<usize>(
        2,
        2,
        &AtomicBool::new(false),
        |ordinal| {
            if ordinal == 0 {
                let (finished, timeout) = finished
                    .1
                    .wait_timeout_while(
                        finished.0.lock().unwrap(),
                        Duration::from_secs(10),
                        |done| !*done,
                    )
                    .unwrap();
                assert!(!timeout.timed_out() && *finished, "second worker never ran");
            }
            completion_order.lock().unwrap().push(ordinal);
            if ordinal == 1 {
                *finished.0.lock().unwrap() = true;
                finished.1.notify_all();
            }
            Err(AppError::input(format!("route-{ordinal}")))
        },
        |_, _| {},
    );
    assert_eq!(*completion_order.lock().unwrap(), vec![1, 0]);
    assert!(result.unwrap_err().to_string().contains("route-0"));
}

#[test]
fn in_flight_cancellation_joins_all_workers_and_discards_prepared_routes() {
    if !crate::test_gates::workers_or_skip("route preparation cancellation", 2) {
        return;
    }
    let cancellation = AtomicBool::new(false);
    let started = (Mutex::new(0usize), Condvar::new());
    let active = AtomicUsize::new(0);
    let finished = AtomicUsize::new(0);
    let progress = RefCell::new(Vec::new());
    let result = map_batches(
        20,
        2,
        &cancellation,
        |ordinal| {
            active.fetch_add(1, Ordering::SeqCst);
            let mut count = started.0.lock().unwrap();
            *count += 1;
            started.1.notify_all();
            let (count, timeout) = started
                .1
                .wait_timeout_while(count, Duration::from_secs(10), |count| *count < 2)
                .unwrap();
            assert!(
                !timeout.timed_out() && *count >= 2,
                "second worker never ran"
            );
            drop(count);
            cancellation.store(true, Ordering::SeqCst);
            finished.fetch_add(1, Ordering::SeqCst);
            active.fetch_sub(1, Ordering::SeqCst);
            Ok(Some(ordinal))
        },
        |completed, _| progress.borrow_mut().push(completed),
    )
    .unwrap();
    assert!(result.is_none());
    assert_eq!(active.load(Ordering::SeqCst), 0);
    assert_eq!(finished.load(Ordering::SeqCst), 2);
    assert_eq!(*progress.borrow(), vec![0]);
}

#[test]
fn cancellation_at_final_progress_cannot_publish_partial_preparation() {
    let cancellation = AtomicBool::new(false);
    let result = map_batches(
        3,
        1,
        &cancellation,
        |ordinal| Ok(Some(ordinal)),
        |completed, _| {
            if completed == 3 {
                cancellation.store(true, Ordering::Relaxed);
            }
        },
    )
    .unwrap();
    assert!(result.is_none());
}

#[test]
fn cancellation_before_progress_or_between_batches_never_dispatches_later_work() {
    let calls = AtomicUsize::new(0);
    let pre_cancelled = AtomicBool::new(true);
    let result = map_batches::<usize>(
        8,
        1,
        &pre_cancelled,
        |_| {
            calls.fetch_add(1, Ordering::Relaxed);
            Ok(None)
        },
        |_, _| panic!("cancelled preparation must not publish progress"),
    )
    .unwrap();
    assert!(result.is_none());
    assert_eq!(calls.load(Ordering::Relaxed), 0);

    let cancellation = AtomicBool::new(false);
    let result = map_batches(
        8,
        1,
        &cancellation,
        |ordinal| {
            calls.fetch_add(1, Ordering::Relaxed);
            Ok(Some(ordinal))
        },
        |completed, _| {
            if completed == 2 {
                cancellation.store(true, Ordering::Relaxed);
            }
        },
    )
    .unwrap();
    assert!(result.is_none());
    assert_eq!(calls.load(Ordering::Relaxed), 2);
}

#[test]
fn route_preparation_rejects_zero_budget_even_for_empty_input() {
    let error =
        map_batches::<usize>(0, 0, &AtomicBool::new(false), |_| Ok(None), |_, _| {}).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("worker-core budget must be positive"),
        "{error}"
    );
}

#[test]
fn native_route_maps_and_exact_transport_coefficients_match_across_widths() {
    let source = r#"
schema="rustred.project.toml.v1"
[family]
name="parallel_route_transport"
loop_momenta=["q1","q2"]
external_momenta=[]
dimension="d"
[[family.denominators]]
id="P1"
expression="q1^2-1"
[[family.denominators]]
id="P2"
expression="q2^2-1"
[[family.denominators]]
id="P3"
expression="(q1-q2)^2-1"
[target]
powers=[2,2,-2]
"#;
    let project =
        crate::application::input::prepare_input(source, crate::InputFormat::Toml).unwrap();
    let (_, _, _, lowered) = crate::application::lowering::lower_project(project)
        .unwrap()
        .into_parts();
    let family = Arc::new(lowered.into_family());
    let route = || {
        serde_json::from_value::<Route>(json!({
            "source_mask":"110","owner_mask":"011","requires_transport":true,
            "source_to_representative":[["1","0"],["0","1"]],
            "owner_to_representative":[["1","-1"],["1","0"]]
        }))
        .unwrap()
    };
    // Four independent verification jobs actually enter the W4 pool. Duplicate
    // witnesses are legal at this internal map-preparation boundary only;
    // final reducer admission still rejects duplicate source supports.
    let routes = [route(), route(), route(), route()];
    let target = rustred::family::IntegralKey::try_new([2, 2, -2]).unwrap();
    let mut baseline = None;
    for workers in [1, 2, 4] {
        if !crate::test_gates::workers_or_skip("parallel native route coefficients", workers) {
            continue;
        }
        let prepared = prepare::<3>(&family, &routes, workers, &AtomicBool::new(false), &|_| {})
            .unwrap()
            .unwrap();
        let transports: Vec<_> = prepared
            .iter()
            .map(|route| {
                route
                    .transport
                    .transport(&target, Default::default())
                    .unwrap()
            })
            .collect();
        assert_eq!(transports.len(), 4);
        assert!(transports.iter().all(|value| !value.terms().is_empty()));
        if let Some(expected) = &baseline {
            assert_eq!(&transports, expected);
        } else {
            baseline = Some(transports);
        }
    }
}
