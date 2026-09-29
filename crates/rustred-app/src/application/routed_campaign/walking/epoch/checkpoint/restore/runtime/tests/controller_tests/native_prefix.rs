//! Real native continuation after cooperative input-prefix interruption only.
use super::native_equivalence::{closed_fixture, completed_snapshot, finish_native};
use super::*;
use crate::application::routed_campaign::walking::epoch::checkpoint::restore::runtime::admission;

#[test]
fn real_native_input_prefix_continuation_preserves_full_graph_and_records() {
    const TEST: &str = "real native epoch input-prefix continuation";
    if !crate::test_gates::workers_or_skip(TEST, 3)
        || !std::thread::spawn(|| crate::test_gates::licensed_or_skip(TEST))
            .join()
            .unwrap()
    {
        return; // Explicit skip remains no native equivalence coverage.
    }
    let mut fixture = closed_fixture();
    let mut baseline = admission::fresh(fixture.directory.0.clone(), &fixture.identity()).unwrap();
    assert_eq!(
        admission::continue_admission(
            &mut baseline,
            &fixture.identity(),
            &fixture.reducer,
            16,
            || None,
            |_| {}
        )
        .unwrap(),
        admission::Outcome::Ready
    );
    finish_native(&fixture, &mut baseline);
    let expected = completed_snapshot(&mut baseline);
    drop(baseline);

    fixture.directory = Directory::new();
    let mut prefix = admission::fresh(fixture.directory.0.clone(), &fixture.identity()).unwrap();
    let mut polls = 0;
    assert_eq!(
        admission::continue_admission(
            &mut prefix,
            &fixture.identity(),
            &fixture.reducer,
            16,
            || {
                let stop_now = polls == 1;
                polls += 1;
                stop_now.then(|| stop::requested(&AtomicBool::new(true), None).unwrap())
            },
            |_| {}
        )
        .unwrap(),
        admission::Outcome::Stopped(merge::StopReason::Paused)
    );
    assert_eq!(prefix.roots.rows.len(), 1);
    assert_eq!(prefix.state.k, 0);
    assert_eq!(prefix.state.counters.dispatched, 0);
    assert_eq!(prefix.records.total(), 0);
    drop(prefix);
    fixture.request.workers = 3;
    let mut resumed = fixture.open().unwrap();
    let session = resumed.dispatch.checkpoint_snapshot().session;
    assert_eq!(
        admission::continue_admission(
            &mut resumed,
            &fixture.identity(),
            &fixture.reducer,
            16,
            || None,
            |_| {}
        )
        .unwrap(),
        admission::Outcome::Ready
    );
    assert_eq!(resumed.dispatch.checkpoint_snapshot().session, session);
    assert_eq!(resumed.roots.rows.len(), 3);
    finish_native(&fixture, &mut resumed);
    assert_eq!(completed_snapshot(&mut resumed), expected);
}
