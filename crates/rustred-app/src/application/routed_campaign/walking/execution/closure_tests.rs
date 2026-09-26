//! Integration through the actual coordinator admission/publication paths.
use super::super::{
    checkpoint::{round_trip_state, test_support::Fixture},
    delegation::{Ledger, SchedulingPolicy},
    queue::Domain,
};
use super::*;
use std::num::NonZeroUsize;

fn request() -> OwnerDomainWalkRequest {
    OwnerDomainWalkRequest::new(super::super::OwnerDomainMatchRequest::new(
        String::new(),
        String::new(),
    ))
}
fn point(phase: Phase, value: u64) -> Domain<1> {
    Domain {
        phase,
        owner: [true],
        lower: vec![value],
        upper: vec![Some(value)],
        rank: None,
        powers: Default::default(),
    }
}
fn finished() -> Finished {
    Finished {
        stats: NativeStats::Apply(Default::default()),
        error: None,
        error_kind: "none",
        seconds: 0.0,
    }
}
fn refresh(state: &State<1>) -> Value {
    state.refresh_closure(&AtomicBool::new(false), true);
    state.closure_json()
}
fn admit(state: &mut State<1>, domain: Domain<1>) {
    state
        .accept(
            Event::one(Effect::Admit {
                domain,
                successor: false,
                conditional: false,
            }),
            &request(),
        )
        .unwrap();
}

#[test]
fn apply_route_exact_and_containment_reuse_all_block_source_roots() {
    let mut queue = Queue::new(100, None);
    queue.admit(point(Phase::Apply, 0)).unwrap();
    let mut containing = point(Phase::Route, 2);
    containing.upper = vec![Some(8)];
    queue.admit(containing.clone()).unwrap();
    let mut state = State::new(queue, 0, None);
    admit(&mut state, containing);
    admit(&mut state, point(Phase::Route, 3));
    state.commit(0, finished());
    assert_eq!(refresh(&state)["dependency_edges"], 1);
    assert_eq!(state.closure_json()["initial_closed"], 0);
    state.commit(
        1,
        Finished {
            stats: NativeStats::Route(Default::default()),
            ..finished()
        },
    );
    assert_eq!(refresh(&state)["initial_closed"], 2);
}

#[test]
fn targeted_initial_reuse_and_job_local_cache_keep_transitive_edges() {
    let mut queue = Queue::new(100, None);
    queue.admit(point(Phase::Apply, 0)).unwrap();
    queue.admit(point(Phase::Apply, 10)).unwrap();
    let mut state = State::new(queue, 0, None);
    state
        .accept(
            Event {
                count: 100,
                effect: Effect::PreAdmittedOrthantReuse {
                    target: 1,
                    successor: true,
                    conditional: true,
                },
            },
            &request(),
        )
        .unwrap();
    let mut cache = super::super::reuse::Cache::new(true);
    for _ in 0..3 {
        assert!(
            cache
                .forward(
                    Event::one(Effect::Admit {
                        domain: point(Phase::Route, 20),
                        successor: true,
                        conditional: false
                    }),
                    &mut |event| {
                        state.accept(event, &request()).unwrap();
                        ControlFlow::Continue(())
                    }
                )
                .is_continue()
        );
    }
    state.commit(0, finished());
    assert_eq!(refresh(&state)["dependency_edges"], 2);
    assert_eq!(state.closure_json()["initial_closed"], 0);
    state.commit(1, finished());
    assert_eq!(refresh(&state)["initial_closed"], 1);
    state.commit(
        2,
        Finished {
            stats: NativeStats::Route(Default::default()),
            ..finished()
        },
    );
    assert_eq!(refresh(&state)["initial_closed"], 2);
    assert_eq!(state.closure_json()["total_closed"], 3);
}

#[test]
fn local_frontier_is_not_transitive_closure_even_with_native_success() {
    let mut queue = Queue::new(10, None);
    queue.admit(point(Phase::Apply, 0)).unwrap();
    let mut state = State::new(queue, 0, None);
    state
        .accept(
            Event::one(Effect::Frontier {
                value: json!({"kind":"test"}),
                successor: false,
                conditional: false,
            }),
            &request(),
        )
        .unwrap();
    state.commit(0, finished());
    assert_eq!(state.completed, 1);
    assert_eq!(refresh(&state)["locally_inspected"], 1);
    assert_eq!(state.closure_json()["total_closed"], 0);
}

#[test]
fn ready_ticket_parent_not_contiguous_cursor_owns_edges_and_checkpoint_replay() {
    let mut queue = Queue::new(10, None);
    queue.delegation = Some(Ledger::new_ready(NonZeroUsize::new(3).unwrap(), 10).unwrap());
    for id in 0..3 {
        queue.admit(point(Phase::Apply, id)).unwrap();
    }
    let mut state = State::new(queue, 0, None);
    state
        .activate_stream(
            Ticket {
                parent: 1,
                part: None,
            },
            true,
        )
        .unwrap();
    let make = || {
        Event::one(Effect::PreAdmittedOrthantReuse {
            target: 2,
            successor: true,
            conditional: false,
        })
    };
    state.accept(make(), &request()).unwrap();
    let fixture = Fixture::save(&state);
    let manifest = fixture.manifest();
    assert_eq!(manifest["format"], "RUSTRED-WALK-CP5");
    assert_eq!(manifest["publication_policy"], "ready");
    let mut resumed = fixture.resume::<1>().unwrap();
    let mut replayed = make();
    assert!(!resumed.filter_replay(&mut replayed).unwrap());
    assert_eq!(refresh(&resumed)["dependency_edges"], 1);
    resumed.note_native_started(1).unwrap();
    resumed.commit(1, finished());
    resumed.complete_stream(Ticket {
        parent: 1,
        part: None,
    });
    assert_eq!(refresh(&resumed)["initial_closed"], 0);
    resumed
        .activate_stream(
            Ticket {
                parent: 2,
                part: None,
            },
            false,
        )
        .unwrap();
    resumed.note_native_started(2).unwrap();
    resumed.commit(2, finished());
    resumed.complete_stream(Ticket {
        parent: 2,
        part: None,
    });
    assert_eq!(refresh(&resumed)["initial_closed"], 2); // Root0 still uninspected.
}

#[test]
fn delegated_alias_waits_for_representative_and_old_checkpoint_magic_is_rejected() {
    let mut queue = Queue::with_policy(
        20,
        None,
        SchedulingPolicy::TransferUnreserved {
            lookahead: NonZeroUsize::new(1).unwrap(),
        },
    )
    .unwrap();
    queue.admit(point(Phase::Apply, 0)).unwrap();
    queue.admit(point(Phase::Apply, 3)).unwrap();
    let mut wider = point(Phase::Apply, 2);
    wider.upper = vec![Some(4)];
    queue.admit(wider).unwrap();
    let mut state = State::new(queue, 0, None);
    state.note_native_started(0).unwrap();
    state.commit(0, finished());
    state.commit_delegated().unwrap();
    assert_eq!(refresh(&state)["total_closed"], 1);
    let fixture = Fixture::save(&state);
    let manifest = fixture.manifest();
    assert_eq!(manifest["format"], "RUSTRED-WALK-CP5");
    assert_eq!(manifest["publication_policy"], "ordered");
    let mut resumed = fixture.resume::<1>().unwrap();
    resumed.note_native_started(2).unwrap();
    resumed.commit(2, finished());
    assert_eq!(refresh(&resumed)["total_closed"], 3);
    // A CP3/CP4 manifest is refused outright, never decoded.
    let mut old = manifest;
    old["schema"] = json!(4);
    old["format"] = json!("RUSTRED-WALK-CP4");
    fixture.write_manifest(&old);
    assert!(
        fixture
            .resume::<1>()
            .err()
            .unwrap()
            .contains("fresh CP5 campaign")
    );
}

#[test]
fn checkpoint_rejects_fictional_seal_and_missing_alias_dependency() {
    let mut queue = Queue::new(10, None);
    queue.admit(point(Phase::Apply, 0)).unwrap();
    let state = State::new(queue, 0, None);
    state.closure.borrow_mut().finish(0, true, true);
    assert!(
        round_trip_state(&state)
            .err()
            .unwrap()
            .contains("unpublished node")
    );

    let mut queue = Queue::with_policy(
        20,
        None,
        SchedulingPolicy::TransferUnreserved {
            lookahead: NonZeroUsize::new(1).unwrap(),
        },
    )
    .unwrap();
    queue.admit(point(Phase::Apply, 0)).unwrap();
    queue.admit(point(Phase::Apply, 3)).unwrap();
    let mut wider = point(Phase::Apply, 2);
    wider.upper = vec![Some(4)];
    queue.admit(wider).unwrap();
    let mut state = State::new(queue, 0, None);
    state.note_native_started(0).unwrap();
    state.commit(0, finished());
    state.commit_delegated().unwrap();
    let mut omitted = super::super::descendant_closure::Tracker::new(3);
    omitted.finish(0, true, true);
    omitted.finish(1, false, true);
    state.closure = RefCell::new(omitted);
    assert!(
        round_trip_state(&state)
            .err()
            .unwrap()
            .contains("edge missing")
    );
}

#[test]
fn partial_initial_inspection_keeps_anchor_frontier_transitively_blocking() {
    let mut queue = Queue::with_policy(
        20,
        None,
        SchedulingPolicy::TransferUnreserved {
            lookahead: NonZeroUsize::new(3).unwrap(),
        },
    )
    .unwrap();
    queue
        .delegation
        .as_mut()
        .unwrap()
        .begin_initial_admission()
        .unwrap();
    queue.admit(point(Phase::Apply, 0)).unwrap();
    queue.admit(point(Phase::Apply, 10)).unwrap();
    queue
        .delegation
        .as_mut()
        .unwrap()
        .finish_initial_admission()
        .unwrap();
    let mut state = State::new(queue, 0, None);
    state.note_native_started(0).unwrap();
    admit(&mut state, point(Phase::Apply, 20));
    state
        .accept(
            Event::one(Effect::Frontier {
                value: json!({"kind":"anchor_frontier"}),
                successor: false,
                conditional: false,
            }),
            &request(),
        )
        .unwrap();
    state.commit(0, finished());
    state.note_native_started(1).unwrap();
    state.commit(1, finished());
    state.note_native_started(2).unwrap();
    let scope = super::super::initial_overlap::InitialOverlapScope {
        anchor_id: 0,
        cut: 1,
        residual_powers: Default::default(),
    };
    state.commit(
        2,
        Finished {
            stats: NativeStats::ApplyPartial(Default::default(), scope),
            ..finished()
        },
    );
    assert!(state.error.is_none());
    assert_eq!(refresh(&state)["initial_closed"], 1);
    assert_eq!(state.closure_json()["total_closed"], 1);
    assert_eq!(state.closure_json()["dependency_edges"], 2);
    let restored = round_trip_state(&state).unwrap();
    assert_eq!(refresh(&restored)["total_closed"], 1);
}
