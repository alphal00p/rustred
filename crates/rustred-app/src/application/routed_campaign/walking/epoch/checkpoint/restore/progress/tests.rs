use super::*;
use std::cell::RefCell;

#[test]
fn bounded_parallel_validation_visits_every_item_once_and_reports_completion() {
    for workers in [1usize, 2, 4] {
        let calls: Vec<_> = (0..97).map(|_| AtomicUsize::new(0)).collect();
        let active = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        let rendezvous = std::sync::Barrier::new(workers.saturating_sub(1).max(1));
        let updates = RefCell::new(Vec::new());
        Control {
            workers,
            cancelled: &|| false,
            observer: &|update| updates.borrow_mut().push(update),
        }
        .parallel("anchor_coverage", calls.len(), &|index| {
            let current = active.fetch_add(1, Ordering::SeqCst) + 1;
            peak.fetch_max(current, Ordering::SeqCst);
            if index < 96 && index % 32 == 0 {
                rendezvous.wait();
            }
            calls[index].fetch_add(1, Ordering::Relaxed);
            std::thread::sleep(Duration::from_millis(1));
            active.fetch_sub(1, Ordering::SeqCst);
            Ok(())
        })
        .unwrap();
        assert!(calls.iter().all(|calls| calls.load(Ordering::Relaxed) == 1));
        assert_eq!(active.load(Ordering::SeqCst), 0);
        assert!(peak.load(Ordering::SeqCst) <= workers.saturating_sub(1).max(1));
        assert_eq!(
            peak.load(Ordering::SeqCst),
            workers.saturating_sub(1).max(1)
        );
        let updates = updates.borrow();
        assert_eq!(updates.first().unwrap().completed, 0);
        let done = updates.last().unwrap();
        assert_eq!(
            (done.stage, done.completed, done.total),
            ("anchor_coverage", 97, 97)
        );
    }
}

#[test]
fn parallel_failure_keeps_the_first_error_even_when_a_later_chunk_fails_first() {
    for workers in [1, 4] {
        let error = Control {
            workers,
            ..Control::serial()
        }
        .parallel("anchor_coverage", 100, &|index| {
            if index == 3 {
                std::thread::sleep(Duration::from_millis(20));
            }
            if matches!(index, 3 | 35) {
                Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    index.to_string(),
                ))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error.to_string(), "3");
        assert!(!operational(&error));
    }
}

#[test]
fn cancellation_joins_workers_and_is_operational() {
    let cancel = AtomicBool::new(false);
    let calls = AtomicUsize::new(0);
    let error = Control {
        workers: 4,
        cancelled: &|| cancel.load(Ordering::Acquire),
        observer: &|_| {},
    }
    .parallel("anchor_coverage", 10_000, &|_| {
        calls.fetch_add(1, Ordering::Relaxed);
        cancel.store(true, Ordering::Release);
        std::thread::sleep(Duration::from_millis(1));
        Ok(())
    })
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::Interrupted);
    assert!(is_cancelled(&error));
    assert!(operational(&error));
    let joined = calls.load(Ordering::Relaxed);
    assert!(joined < 10_000);
    std::thread::sleep(Duration::from_millis(5));
    assert_eq!(calls.load(Ordering::Relaxed), joined);
}

#[test]
fn plain_io_interruption_does_not_claim_cancellation_before_adoption() {
    let error = io::Error::from(io::ErrorKind::Interrupted);
    assert!(operational(&error));
    assert!(!is_cancelled(&error));
    let error = Control {
        cancelled: &|| true,
        ..Control::serial()
    }
    .check()
    .unwrap_err();
    assert!(operational(&error));
    assert!(is_cancelled(&error));
}

#[test]
fn panicking_worker_is_joined_and_does_not_allow_checkpoint_fallback() {
    let error = Control {
        workers: 4,
        ..Control::serial()
    }
    .parallel("anchor_coverage", 100, &|index| {
        assert_ne!(index, 1, "test worker failure");
        Ok(())
    })
    .unwrap_err();
    assert!(operational(&error));
    assert!(error.to_string().contains("worker panicked"));
}
