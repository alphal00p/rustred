//! Lane closeloop3 (closeloop2's request "FOR THE LEDGER OWNER"; interfaces/closeloop.md 16.5): a node registered with
//! `spec.start_target: boxed` is BOXED from its first ledger record. Pure reducer tests (no process, no file).

mod ledger_support;

use ledger_support::*;
use rustred_campaign::ledger::{Event, NodeSpecValues, Record, StartTarget};
use rustred_campaign::state::{
    AttemptEndCause, Blockers, CampaignState, ClosureTarget, EvidenceLevel, NodeEvent, NodePhase,
};

const N: &str = "N0";

fn node(ev: NodeEvent) -> Event {
    Event::Node {
        node: N.into(),
        ev,
    }
}

/// genesis + the standard setup with the node's spec changed by `f`.
fn events(f: impl Fn(&mut Event)) -> Vec<Event> {
    let mut v = vec![genesis_event()];
    let mut s = setup_events(N);
    for e in s.iter_mut() {
        f(e);
    }
    v.extend(s);
    v
}

fn boxed_start(e: &mut Event) {
    if let Event::NodeRegistered { spec, .. } = e {
        spec.start_target = Some(StartTarget::Boxed);
    }
}

fn apply(st: &mut CampaignState, seq: u64, e: &Event) -> Result<(), String> {
    let r = Record {
        schema: rustred_campaign::ledger::LEDGER_SCHEMA.into(),
        seq,
        utc: "2026-11-14T00:00:00.000Z".into(),
        prev: rustred_campaign::ledger::GENESIS.into(),
        event: e.clone(),
    };
    st.apply_record(&r).map_err(|e| e.to_string())
}

#[test]
fn a_node_registered_boxed_starts_on_its_spec_box_and_closes_boxed() {
    let mut ev = events(boxed_start);
    let st = reduce_events(&ev);
    let n = &st.nodes[N];
    assert_eq!(
        n.state.target,
        ClosureTarget::Boxed {
            box_id: format!("{N}.box")
        }
    );
    assert_eq!(n.state.phase, NodePhase::Running);
    assert!(!n.state.full_closed_ever);
    // a box check that closes at once: closed BOXED (impossible before: no FULL attempt had ended)
    ev.extend(item_life(N, 0, 0, EvidenceLevel::IdentityProved, 10));
    let st = reduce_events(&ev);
    ev.extend(closure_events(&st, N, 1, 0));
    ev.push(node(NodeEvent::DeclareClosed));
    let st = reduce_events(&ev);
    let n = &st.nodes[N];
    assert_eq!(n.state.phase, NodePhase::Closed);
    assert_eq!(n.state.closure_kind(), "boxed");
    assert!(!n.state.full_closed_ever, "a node registered boxed was never FULL");
    // the same box again is a no-op repeat; another box is refused
    let mut st2 = st.clone();
    let seq = ev.len() as u64 + 1;
    let mut ev_run = ev.clone();
    ev_run.push(node(NodeEvent::Reopen {
        reason: "test".into(),
    }));
    let st3 = reduce_events(&ev_run);
    // a reopen returns it to its START target (its box), never to FULL
    assert_eq!(
        st3.nodes[N].state.target,
        ClosureTarget::Boxed {
            box_id: format!("{N}.box")
        }
    );
    assert_eq!(st3.nodes[N].state.phase, NodePhase::Running);
    assert!(
        apply(
            &mut st2,
            seq,
            &node(NodeEvent::FallBackToBox {
                box_id: "another.box".into()
            })
        )
        .is_err()
    );
}

#[test]
fn a_boxed_start_without_a_spec_box_is_refused() {
    let ev = events(|e| {
        if let Event::NodeRegistered { spec, spec_box, .. } = e {
            spec.start_target = Some(StartTarget::Boxed);
            *spec_box = None;
        }
    });
    let mut st = CampaignState::default();
    let mut refused = None;
    for (i, e) in ev.iter().enumerate() {
        if let Err(why) = apply(&mut st, i as u64 + 1, e) {
            refused = Some((e.kind(), why));
            break;
        }
    }
    let (kind, why) = refused.expect("the registration is refused");
    assert_eq!(kind, "node_registered");
    assert!(why.contains("start_target boxed needs a spec box"), "{why}");
}

#[test]
fn an_open_boxed_start_node_is_declared_open_without_any_fallback() {
    let mut ev = events(boxed_start);
    ev.extend(item_life(N, 0, 0, EvidenceLevel::IdentityProved, 10));
    let st = reduce_events(&ev);
    ev.extend(closure_events(&st, N, 1, 3));
    // the spec's fixer-round budget (2) ends the attempt on the box; no FallBackToBox is needed or legal-required
    ev.push(node(NodeEvent::FixerRoundBegin {
        round: 1,
        partition: None,
    }));
    ev.push(node(NodeEvent::FixerRoundBegin {
        round: 2,
        partition: None,
    }));
    ev.push(node(NodeEvent::AttemptEnded {
        blockers: Blockers {
            faces: 0,
            loci: 0,
            keys: 3,
            list: None,
        },
        reason: "budget".into(),
        cause: AttemptEndCause::FixerRounds,
    }));
    ev.push(node(NodeEvent::DeclareOpen {
        reason: "3 keys open".into(),
    }));
    let st = reduce_events(&ev);
    assert_eq!(st.nodes[N].state.phase, NodePhase::Open);
    assert!(matches!(st.nodes[N].state.target, ClosureTarget::Boxed { .. }));
}

#[test]
fn records_without_start_target_serialize_as_before() {
    // the field is written only when given: an older record's canonical form is unchanged
    let v = serde_json::to_value(NodeSpecValues::default()).unwrap();
    assert!(v.get("start_target").is_none(), "{v}");
    let b = serde_json::to_value(NodeSpecValues {
        start_target: Some(StartTarget::Boxed),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(b["start_target"], "boxed");
    let back: NodeSpecValues = serde_json::from_value(b).unwrap();
    assert_eq!(back.start_target, Some(StartTarget::Boxed));
    // the default registration is FULL, as before
    let st = reduce_events(&events(|_| {}));
    assert_eq!(st.nodes[N].state.target, ClosureTarget::Full);
}
