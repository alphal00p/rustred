//! Integration through the actual coordinator admission/publication paths.
use super::super::{
    checkpoint::{
        SaveKind,
        manifest::Section,
        round_trip_state,
        test_support::{Fixture, OWNER},
    },
    delegation::{Ledger, SchedulingPolicy},
    descendant_closure::Tracker,
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

/// Initial prefix {0, 10} under a ledger; native 0 retains a frontier and
/// discovers 2, which is inspected as a partial initial overlap anchored at 0.
fn partial_initial_state() -> State<1> {
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
    state
}

#[test]
fn partial_initial_inspection_keeps_anchor_frontier_transitively_blocking() {
    let state = partial_initial_state();
    assert_eq!(refresh(&state)["initial_closed"], 1);
    assert_eq!(state.closure_json()["total_closed"], 1);
    assert_eq!(state.closure_json()["dependency_edges"], 2);
    let restored = round_trip_state(&state).unwrap();
    assert_eq!(refresh(&restored)["total_closed"], 1);
}

fn frontier(state: &mut State<1>) {
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
}
/// The state's dependency monitor rebuilt from its per-ID status and the
/// edges `keep` retains; nothing refreshed, so the closed counts are zero.
fn rebuilt_closure(state: &State<1>, keep: impl Fn((usize, usize)) -> bool) -> Tracker {
    let original = state.closure.borrow();
    let mut rebuilt = Tracker::new(state.initial_domain_count);
    rebuilt.discovered(state.queue.domains.len());
    for edge in original.dependencies().filter(|&edge| keep(edge)) {
        rebuilt.edge(edge.0, edge.1);
    }
    for id in 0..state.queue.domains.len() {
        let (inspected, sealed) = original.local_status(id).unwrap();
        rebuilt.finish(id, inspected, sealed);
    }
    rebuilt
}

#[test]
fn checkpoint_rejects_a_seal_the_ledger_or_frontier_count_does_not_allow() {
    // Ledger: a published native that retained a frontier is never sealed.
    let mut queue = Queue::with_policy(
        20,
        None,
        SchedulingPolicy::TransferUnreserved {
            lookahead: NonZeroUsize::new(1).unwrap(),
        },
    )
    .unwrap();
    queue.admit(point(Phase::Apply, 0)).unwrap();
    let mut state = State::new(queue, 0, None);
    state.note_native_started(0).unwrap();
    frontier(&mut state);
    state.commit(0, finished());
    assert!(round_trip_state(&state).is_ok());
    let mut sealed = Tracker::new(1);
    sealed.finish(0, true, true);
    state.closure = RefCell::new(sealed);
    let error = round_trip_state(&state).err().unwrap();
    assert!(
        error.contains("seal disagrees with native publication"),
        "{error}"
    );

    // No ledger: the seal of an inspection depends on its record's frontiers,
    // so only the number of unsealed inspections is bounded by the frontier
    // count. A native without frontiers may not stay unsealed...
    let mut queue = Queue::new(10, None);
    queue.admit(point(Phase::Apply, 0)).unwrap();
    let mut state = State::new(queue, 0, None);
    state.commit(0, finished());
    assert_eq!(state.frontiers, 0);
    assert!(round_trip_state(&state).is_ok());
    let mut unsealed = Tracker::new(1);
    unsealed.finish(0, true, false);
    state.closure = RefCell::new(unsealed);
    let error = round_trip_state(&state).err().unwrap();
    assert!(
        error.contains("seal disagrees with native publication"),
        "{error}"
    );
    // ...while one retained frontier admits exactly one unsealed inspection.
    let mut queue = Queue::new(10, None);
    queue.admit(point(Phase::Apply, 0)).unwrap();
    let mut state = State::new(queue, 0, None);
    frontier(&mut state);
    state.commit(0, finished());
    assert_eq!(state.closure.borrow().local_status(0), Some((true, false)));
    assert!(round_trip_state(&state).is_ok());
}

#[test]
fn checkpoint_rejects_a_missing_or_misplaced_partial_anchor() {
    let mut state = partial_initial_state();
    let anchor = (2, 0);
    assert!(
        state
            .closure
            .borrow()
            .dependencies()
            .any(|edge| edge == anchor)
    );
    // The rebuild itself is a valid monitor; without the anchor edge it is not.
    state.closure = RefCell::new(rebuilt_closure(&state, |_| true));
    assert!(round_trip_state(&state).is_ok());
    state.closure = RefCell::new(rebuilt_closure(&state, |edge| edge != anchor));
    assert_eq!(state.closure.borrow().edge_count(), 1);
    let error = round_trip_state(&state).err().unwrap();
    assert!(error.contains("partial-anchor edge missing"), "{error}");

    // An anchor at or beyond the initial prefix. The ledger restore already
    // refuses an anchor outside its protected prefix, so shrink the prefix
    // the meta section records instead (with every counter bound to it).
    let state = partial_initial_state();
    let fixture = Fixture::save(&state);
    fixture.rewrite_section::<1>(Section::Meta, |meta| {
        meta["counters"][10] = json!(0); // initial_domain_count
        meta["counters"][11] = json!(0); // initial_entry_domains_inspected
        meta["closure"]["initial"] = json!(0);
        meta["closure"]["initial_closed"] = json!(0);
    });
    let error = fixture.resume::<1>().err().unwrap();
    assert!(
        error.contains("partial anchor outside initial prefix"),
        "{error}"
    );
}

#[test]
fn exhausted_walk_reports_a_current_closure_despite_a_late_stop_request() {
    let mut queue = Queue::new(10, None);
    queue.admit(point(Phase::Apply, 0)).unwrap();
    let mut state = State::new(queue, 0, None);
    admit(&mut state, point(Phase::Apply, 1));
    state.commit(0, finished());
    let stop = AtomicBool::new(true);
    let cut = |state: &State<1>| std::ptr::eq(state.report_cancellation(&stop), &stop);
    assert!(
        cut(&state),
        "an interrupted walk keeps the run's cancellation"
    );
    let fixture = Fixture::save(&state);
    state.commit(1, finished());
    state.checkpoint_paused = true;
    assert!(cut(&state), "so does a pause");
    state.checkpoint_paused = false;
    assert!(!cut(&state));
    // The stop request arrived after the last publication: the report
    // refresh at the end of `walking::run` still scans.
    state.refresh_closure(&stop, true);
    assert_eq!(state.closure_json()["snapshot_stale"], true);
    state.refresh_closure(state.report_cancellation(&stop), true);
    let report = state.closure_json();
    assert_eq!(report["snapshot_stale"], false);
    assert_eq!(report["total_closed"], 2);
    assert_eq!(state.closure.borrow().closed(0), Some(true));
    // So does the final save, which persists that snapshot: a save's scan
    // is never cut, whatever the run's cancellation says.
    let mut resumed = fixture.resume::<1>().unwrap();
    resumed.commit(1, finished());
    let mut store = fixture.open(true).unwrap();
    store.bind_owners(vec![OWNER.into()]).unwrap();
    store
        .save_cancellable(&resumed, &[], &[], SaveKind::Final, &stop, &|_| {})
        .unwrap()
        .unwrap();
    drop(store);
    let persisted = fixture.resume::<1>().unwrap().closure_json();
    assert_eq!(persisted["snapshot_stale"], false);
    assert_eq!(persisted["total_closed"], 2);
}

#[test]
fn saves_under_a_stop_request_persist_a_current_closure_snapshot() {
    // 102adcc3 scanned before every save with a never-set flag, so a stop
    // request must not change what a generation persists; least of all the
    // paused generation that the walk's final save writes.
    let mut queue = Queue::new(10, None);
    for value in [0, 5, 6] {
        queue.admit(point(Phase::Apply, value)).unwrap();
    }
    let mut state = State::new(queue, 0, None);
    admit(&mut state, point(Phase::Apply, 1)); // 0 -> 3 keeps 0 and 3 open.
    let fixture = Fixture::save(&state);
    let stop = AtomicBool::new(true);
    let save = |state: &State<1>, kind| {
        let mut store = fixture.open(true).unwrap();
        store.bind_owners(vec![OWNER.into()]).unwrap();
        store
            .save_cancellable(state, &[], &[], kind, &stop, &|_| {})
            .unwrap()
            .expect("the state changed since the previous generation");
    };
    let closed = |report: &Value| {
        (
            report["snapshot_stale"].clone(),
            report["total_closed"].clone(),
            report["initial_closed"].clone(),
        )
    };
    state.commit(0, finished());
    state.commit(1, finished());
    // The in-loop refresh is cut by the stop request: stale in memory.
    state.refresh_closure(&stop, true);
    assert_eq!(state.closure_json()["snapshot_stale"], true);
    save(&state, SaveKind::Forced);
    let persisted = fixture.resume::<1>().unwrap().closure_json();
    assert_eq!(closed(&persisted), (json!(false), json!(1), json!(1)));
    state.commit(2, finished());
    state.checkpoint_paused = true;
    assert!(std::ptr::eq(state.report_cancellation(&stop), &stop));
    save(&state, SaveKind::Final);
    let persisted = fixture.resume::<1>().unwrap().closure_json();
    assert_eq!(closed(&persisted), (json!(false), json!(2), json!(2)));
    assert_eq!(fixture.manifest()["metadata"]["paused"], true);

    // A save short of scratch memory keeps the previous snapshot; restore
    // accepts such a stale-but-valid one (closed nodes stay closed).
    fixture.rewrite_section::<1>(Section::Meta, |meta| {
        let revision = meta["closure"]["revision"].as_u64().unwrap();
        meta["closure"]["snapshot_revision"] = json!(revision - 1);
    });
    let restored = fixture.resume::<1>().unwrap().closure_json();
    assert_eq!(restored["available"], true);
    assert_eq!(closed(&restored), (json!(true), json!(2), json!(2)));
}
