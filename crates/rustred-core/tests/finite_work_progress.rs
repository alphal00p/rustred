//! Public preparation telemetry has no checkpoint/closure authority.
use rustred::persistence::finite_work::GeneratedWorkProgress;

#[test]
fn finite_work_progress_tracks_units_stages_and_live_polling() {
    let handle = GeneratedWorkProgress::default();
    let consumer = handle.clone();
    let mut stage = GeneratedWorkProgress::begin(Some(&handle), "sort_keys", None, "comparisons");
    stage.finish(1234);
    let snapshot = consumer.snapshot().unwrap();
    assert_eq!((snapshot.completed, snapshot.total), (1234, None));
    assert!(snapshot.finished);
    let mut old = GeneratedWorkProgress::begin(Some(&handle), "records", Some(10), "records");
    let mut current = GeneratedWorkProgress::begin(Some(&handle), "records", Some(20), "records");
    old.finish(10);
    assert_eq!(consumer.snapshot().unwrap().completed, 0);
    current.finish(20);
    assert_eq!(consumer.snapshot().unwrap().completed, 20);
    handle.clear();
    assert!(consumer.snapshot().is_none());
    let mut disabled = GeneratedWorkProgress::begin(None, "noop", None, "records");
    disabled.advance(16384);
    disabled.finish(16384);
    assert!(consumer.snapshot().is_none());
}
