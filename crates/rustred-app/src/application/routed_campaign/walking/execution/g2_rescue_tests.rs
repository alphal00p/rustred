//! G2/rescue boundaries on the real queue, publication ledger, dependency
//! tracker and checkpoint store. These one-coordinate fixtures test scheduler
//! authority, not integral-family completeness or native CAS performance.
//! Quarantine is installed at the same state boundary as rescue; full request
//! binding and append-only amendment parsing remain covered by rescue tests.
use super::super::{
    checkpoint::test_support::{Fixture, OWNER},
    delegation::Ledger,
    g2::{G2ResidualAnchors, Outcome, Plan},
    queue::Domain,
};
use super::*;
use std::num::NonZeroUsize;

fn interval(lo: u64, hi: u64) -> Domain<1> {
    Domain {
        phase: Phase::Apply,
        owner: [true],
        lower: vec![lo],
        upper: vec![Some(hi)],
        rank: None,
        powers: Default::default(),
    }
}

fn admit(state: &mut State<1>, domain: Domain<1>) -> usize {
    let expected = state.queue.domains.len();
    assert_eq!(state.queue.admit(domain).unwrap(), (expected, true));
    state.closure.borrow_mut().discovered(expected + 1);
    expected
}

fn publish(state: &mut State<1>, id: usize, plan: Option<&Plan>, frontier: bool) {
    assert_eq!(state.queue.next, id);
    state
        .queue
        .delegation
        .as_mut()
        .unwrap()
        .native_started(id)
        .unwrap();
    if frontier {
        state.frontiers += 1;
        state.details.push(json!({"kind":"test_unresolved_child"}));
    }
    state.commit(
        id,
        Finished {
            stats: match plan {
                Some(plan) => NativeStats::ApplyG2(Default::default(), plan.scope()),
                None => NativeStats::Apply(Default::default()),
            },
            error: None,
            error_kind: "none",
            seconds: 0.0,
        },
    );
    assert!(state.error.is_none(), "{:?}", state.error);
}

/// A (id 0) is locally discharged but depends on pending B (id 2). Q
/// (id 3) strictly extends A, so it is admitted and can borrow A's low band.
/// R (id 1) has already published its dependency on Q, before Q is planned.
fn pending_child(g2: bool) -> State<1> {
    let mut queue = Queue::new(usize::MAX, None);
    queue.delegation = Some(Ledger::new(NonZeroUsize::MIN, usize::MAX).unwrap());
    assert_eq!(queue.admit(interval(0, 1)).unwrap(), (0, true));
    assert_eq!(queue.admit(interval(20, 20)).unwrap(), (1, true));
    let mut state = State::new(queue, 0, None);
    if g2 {
        state.queue.delegation.as_mut().unwrap().enable_g2(2);
        state.g2_setup().unwrap();
    }
    assert_eq!(admit(&mut state, interval(10, 10)), 2);
    assert_eq!(admit(&mut state, interval(0, 2)), 3);
    state.closure.borrow_mut().edge(0, 2);
    state.closure.borrow_mut().edge(1, 3);
    publish(&mut state, 0, None, false);
    publish(&mut state, 1, None, false);
    state
}

fn install_frontier_quarantine(state: &mut State<1>, expected: &[usize]) {
    let bits = state.closure.borrow().tainted().unwrap();
    assert_eq!(
        state.queue.install_quarantine(bits).unwrap(),
        expected.len()
    );
    for id in 0..state.queue.domains.len() {
        assert_eq!(
            state.queue.is_quarantined(id),
            expected.contains(&id),
            "id={id}"
        );
    }
    // The saved original worker view is kept for historical accepted work.
    state.rescue_worker_epochs = vec![
        vec![],
        (0..state.initial_domain_count)
            .filter(|&id| state.queue.is_quarantined(id))
            .collect(),
    ];
    for id in state.accepted_prefix_holders() {
        state.rescue_holder_epochs.entry(id).or_insert(0);
    }
}

fn assert_fresh_query_has_no_anchor(state: &mut State<1>) {
    let id = admit(state, interval(0, 3));
    let outcome =
        state
            .g2
            .as_ref()
            .unwrap()
            .decide(id, &state.queue.domain(id), &AtomicBool::new(false));
    assert_eq!(*outcome, Outcome::Whole);
}

#[test]
fn g2_then_rescue_replays_durable_zero_callback_pin_and_keeps_blocked_dependency() {
    let mut state = pending_child(true);
    let outcome =
        state
            .g2
            .as_ref()
            .unwrap()
            .decide(3, &state.queue.domain(3), &AtomicBool::new(false));
    let Outcome::Planned(plan) = &*outcome else {
        panic!("A must cover Q's low band")
    };
    let plan = plan.clone();
    assert_eq!(plan.residual, Some((3, 3)));
    assert_eq!(
        plan.anchors.iter().map(|a| a.id).collect::<Vec<_>>(),
        vec![0]
    );
    // No callback from Q has been accepted. A durable pin alone nevertheless
    // fixes its decision; later rescue must not silently replan its stream.
    assert_eq!(state.events, 0);
    publish(&mut state, 2, None, true);
    let mut request = Fixture::request_for(&state);
    request.g2_residual_anchors = G2ResidualAnchors::Union;
    let fixture = Fixture::save_with(&state, request, &[], &[]);
    let mut restored: State<1> = fixture.resume().unwrap();
    // Hook planned by the integration slice: durable pins add dependency
    // evidence before rescue computes reverse taint, including old R -> Q.
    restored.g2_restore_pin_dependencies().unwrap();
    assert!(restored.accepted_prefix_holders().contains(&3));
    let edges = restored.closure.borrow().edge_count();
    restored.g2_restore_pin_dependencies().unwrap();
    assert_eq!(restored.closure.borrow().edge_count(), edges);
    install_frontier_quarantine(&mut restored, &[0, 1, 2, 3]);
    restored.g2_setup().unwrap();
    // A second interruption before any replay must retain the pin, cursor
    // and newly materialized dependencies through the actual binary store.
    let before_pin = restored.g2_pins_json().unwrap();
    let cursor = restored.queue.next;
    let amended = Fixture::save_with(&restored, fixture.request.clone(), &[], &[]);
    let mut restored: State<1> = amended.resume().unwrap();
    assert_eq!(restored.queue.next, cursor);
    assert_eq!(restored.events, 0);
    assert_eq!(restored.closure.borrow().edge_count(), edges);
    assert_eq!(restored.g2_pins_json().unwrap(), before_pin);
    assert_eq!(restored.rescue_holder_epochs.get(&3), Some(&0));
    // Quarantine is recomputed by rescue on every resume; it is deliberately
    // not a second persisted source of coverage or eligibility authority.
    restored.g2_restore_pin_dependencies().unwrap();
    install_frontier_quarantine(&mut restored, &[0, 1, 2, 3]);
    restored.g2_setup().unwrap();
    let replayed =
        restored
            .g2
            .as_ref()
            .unwrap()
            .decide(3, &restored.queue.domain(3), &AtomicBool::new(false));
    assert_eq!(*replayed, Outcome::Planned(plan.clone()));
    assert_fresh_query_has_no_anchor(&mut restored);
    publish(&mut restored, 3, Some(&plan), false);
    restored.refresh_closure(&AtomicBool::new(false), true);
    let closure = restored.closure.borrow();
    assert!(closure.dependencies().any(|edge| edge == (0, 2)));
    assert!(closure.dependencies().any(|edge| edge == (1, 3)));
    assert!(closure.dependencies().any(|edge| edge == (3, 0)));
    assert_eq!(closure.closed(1), Some(false));
    assert_eq!(closure.closed(3), Some(false));
    let ledger = restored.queue.delegation.as_ref().unwrap();
    let row = ledger.g2_row(3).unwrap();
    assert_eq!(ledger.g2().unwrap().anchors_of(row), &[0]);
}

#[test]
fn rescue_then_g2_backfill_excludes_tainted_locally_discharged_anchor() {
    let mut state = pending_child(false);
    publish(&mut state, 2, None, true);
    let fixture = Fixture::save(&state);
    // Keep the actual restored sidecar: g2_backfill consumes its publication
    // stream, rather than a hand-written second model of the activation log.
    let mut store = fixture.open(true).unwrap();
    store.bind_owners(vec![OWNER.into()]).unwrap();
    let mut restored = store.resume::<1>(&|_| {}).unwrap().unwrap().state;
    install_frontier_quarantine(&mut restored, &[0, 2]);
    let activated = restored.g2_backfill().unwrap();
    assert_eq!(activated["backfilled_apply_natives"], 3);
    restored.g2_setup().unwrap();
    // The already pending pre-activation stream remains pinned Whole.
    let original =
        restored
            .g2
            .as_ref()
            .unwrap()
            .decide(3, &restored.queue.domain(3), &AtomicBool::new(false));
    assert_eq!(*original, Outcome::Whole);
    assert!(
        restored
            .queue
            .delegation
            .as_ref()
            .unwrap()
            .discharged_locally(0)
    );
    assert_fresh_query_has_no_anchor(&mut restored);
}

#[test]
fn newly_dead_domain_is_not_a_fresh_anchor_at_publication_or_rebuild() {
    let mut state = pending_child(true);
    // A new dead descendant can finish during replay of its old producer.
    // Its local success is not permission to lend it to a fresh live job.
    state.queue.mark_dead(2);
    publish(&mut state, 2, None, false);
    let candidate = interval(10, 10);
    for rebuild in [false, true] {
        if rebuild {
            state.g2_setup().unwrap();
        }
        let outcome = state
            .g2
            .as_ref()
            .unwrap()
            .decide(4, &candidate, &AtomicBool::new(false));
        assert_eq!(*outcome, Outcome::Whole, "rebuild={rebuild}");
    }
}

#[test]
fn restored_pin_dependencies_validate_the_entire_set_before_mutating_graph() {
    let mut state = pending_child(true);
    let outcome =
        state
            .g2
            .as_ref()
            .unwrap()
            .decide(3, &state.queue.domain(3), &AtomicBool::new(false));
    let Outcome::Planned(plan) = &*outcome else {
        panic!("expected residual plan")
    };
    publish(&mut state, 2, None, true);
    let other = admit(&mut state, interval(0, 3));
    let mut invalid = plan.clone();
    invalid.anchors[0].id = u32::MAX;
    state.g2_pins = vec![
        (3, Outcome::Planned(plan.clone())),
        (other, Outcome::Planned(invalid)),
    ];
    let before_pins = state.g2_pins.clone();
    let before_edges = state.closure.borrow().edge_count();
    assert!(state.g2_restore_pin_dependencies().is_err());
    assert_eq!(state.closure.borrow().edge_count(), before_edges);
    assert_eq!(state.g2_pins, before_pins);
    assert!(
        !state
            .closure
            .borrow()
            .dependencies()
            .any(|edge| edge == (3, 0))
    );
}

#[test]
fn inert_whole_pins_allow_backfilled_route_and_already_published_targets() {
    let mut state = pending_child(true);
    let route = admit(
        &mut state,
        Domain {
            phase: Phase::Route,
            ..interval(0, 2)
        },
    );
    assert!(state.queue.delegation.as_ref().unwrap().is_published(0));
    state.g2_pins = vec![(0, Outcome::Whole), (route, Outcome::Whole)];
    let before = state.g2_pins.clone();
    let edges = state.closure.borrow().edge_count();
    state.g2_restore_pin_dependencies().unwrap();
    assert_eq!(state.closure.borrow().edge_count(), edges);
    assert_eq!(state.g2_pins, before);
}
