//! Serial native control: Ready must degenerate to a resumable ordinary walk.
use super::super::{
    delegation::{Ledger, SchedulingPolicy},
    queue::Domain,
};
use super::*;
use std::num::NonZeroUsize;

fn seed() -> State<1> {
    let mut queue = Queue::new(100, None);
    queue.delegation = Some(Ledger::new_ready(NonZeroUsize::new(2).unwrap(), 100).unwrap());
    queue
        .delegation
        .as_mut()
        .unwrap()
        .begin_initial_admission()
        .unwrap();
    for n in 0..4 {
        queue
            .admit(Domain {
                phase: Phase::Apply,
                owner: [true],
                lower: vec![n],
                upper: vec![Some(n)],
                rank: Some(0),
                powers: Default::default(),
            })
            .unwrap();
    }
    queue
        .delegation
        .as_mut()
        .unwrap()
        .finish_initial_admission()
        .unwrap();
    State::new(queue, 0, None)
}

fn without_timing(mut value: Value) -> Value {
    if let Value::Array(records) = &mut value {
        for record in records {
            record.as_object_mut().unwrap().remove("seconds");
        }
    }
    value
}

#[test]
fn ready_w1_native_disk_resume_preserves_accepted_work_and_master_boundaries() {
    let reducer = super::initial_orthants_tests::native_fixture();
    let mut request = OwnerDomainWalkRequest::new(super::super::OwnerDomainMatchRequest::new(
        String::new(),
        String::new(),
    ));
    request.workers = 1;
    request.publication_policy = OwnerDomainWalkPublicationPolicy::Ready;
    request.scheduling_policy = SchedulingPolicy::TransferUnreserved {
        lookahead: NonZeroUsize::new(2).unwrap(),
    };
    let mut baseline = seed();
    run_checkpointed(
        &mut baseline,
        &reducer,
        &request,
        &AtomicBool::new(false),
        &|_| {},
        &mut |_| Ok(()),
    );
    assert_eq!(baseline.error, None);
    let mut resumed = seed();
    for expected in 1..=2 {
        let cancel = AtomicBool::new(false);
        run_checkpointed(
            &mut resumed,
            &reducer,
            &request,
            &cancel,
            &|_| {},
            &mut |state| {
                if state.completed >= expected {
                    cancel.store(true, Ordering::Release);
                }
                Ok(())
            },
        );
        assert_eq!(resumed.error, None);
        assert!(resumed.checkpoint_paused);
        assert_eq!(resumed.completed, expected);
        assert!(resumed.queue.next < resumed.queue.domains.len());
        resumed = super::super::checkpoint::round_trip_state_on_disk(&resumed, &request).unwrap();
        assert_eq!(resumed.completed, expected);
        assert_eq!(resumed.published_count(), expected);
    }
    run_checkpointed(
        &mut resumed,
        &reducer,
        &request,
        &AtomicBool::new(false),
        &|_| {},
        &mut |_| Ok(()),
    );
    assert_eq!(resumed.error, None);
    assert_eq!(resumed.frontiers, 0);
    assert_eq!(resumed.published_count(), resumed.queue.domains.len());
    assert_eq!(resumed.initial_published(), 4);
    assert_eq!(resumed.pending_descendants(), 0);
    assert_eq!(resumed.queue.domains, baseline.queue.domains);
    assert_eq!(
        without_timing(json!(resumed.records)),
        without_timing(json!(baseline.records))
    );
    assert_eq!(
        (
            resumed.events,
            resumed.successors,
            resumed.completed,
            resumed.queue.deduplicated
        ),
        (
            baseline.events,
            baseline.successors,
            baseline.completed,
            baseline.queue.deduplicated
        )
    );
    assert_eq!(
        resumed.finalize_delegation().unwrap(),
        baseline.finalize_delegation().unwrap()
    );
}
