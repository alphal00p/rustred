//! Replay idempotence (duplicated intent/done pairs and duplicated effect records give the same state) and
//! stale marking of dependent rows when an input changes (marked, never deleted).
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code
mod ledger_support;

use ledger_support::*;
use rustred_campaign::ledger::{
    self, EnvelopeKind, Event, ItemEvent, JobEvent, LedgerConfig, Record,
};
use rustred_campaign::state::{
    CampaignState, EvidenceLevel, ItemPhase, NodeEvent, NodePhase, reduce,
};

fn normalised(mut s: CampaignState) -> CampaignState {
    s.last_seq = 0;
    s.last_utc.clear();
    s
}

/// The state without the bookkeeping that depends on record numbering and stamps (`seq`, `*_seq`, `utc`,
/// `*_utc`): a ledger with duplicated records numbers its records differently, nothing else may differ.
fn semantic(s: &CampaignState) -> serde_json::Value {
    fn strip(v: &mut serde_json::Value) {
        match v {
            serde_json::Value::Object(o) => {
                o.retain(|k, _| {
                    !(k == "seq" || k.ends_with("_seq") || k == "utc" || k.ends_with("_utc"))
                });
                o.values_mut().for_each(strip);
            }
            serde_json::Value::Array(a) => a.iter_mut().for_each(strip),
            _ => {}
        }
    }
    let mut v = serde_json::to_value(s).unwrap();
    strip(&mut v);
    v
}

/// Records that may legitimately be repeated after a crash (the writer re-issues them), and their repeats.
fn repeatable(e: &Event) -> bool {
    matches!(
        e,
        Event::Intent { .. }
            | Event::Done { .. }
            | Event::InputRegistered { .. }
            | Event::NodeRegistered { .. }
            | Event::ItemCreated { .. }
            | Event::JobQueued { .. }
            | Event::EvidenceImported { .. }
            | Event::Job { .. }
            | Event::Item { .. }
            | Event::Node {
                ev: NodeEvent::ClosureResult { .. },
                ..
            }
    )
}

#[test]
fn duplicated_records_reduce_to_the_same_state() {
    let base = scenario();
    let d1 = scratch("replay1");
    let _c1 = Cleanup(d1.clone());
    let clk = clock();
    let mut l1 = new_ledger(&d1, as_dyn(&clk), LedgerConfig::default());
    l1.append_all(base.clone()).unwrap();
    let want = semantic(l1.state());

    // Same events, every repeatable record written twice in a row, and every intent/done PAIR repeated
    // after the done.
    let d2 = scratch("replay2");
    let _c2 = Cleanup(d2.clone());
    let mut l2 = new_ledger(&d2, as_dyn(&clk), LedgerConfig::default());
    let mut dup = 0;
    let mut last_intent: Option<Event> = None;
    for e in base {
        l2.append(e.clone()).unwrap();
        if let Event::Intent { .. } = e {
            last_intent = Some(e.clone());
        }
        if repeatable(&e) {
            l2.append(e.clone())
                .unwrap_or_else(|err| panic!("repeat of {e:?} refused: {err}"));
            dup += 1;
        }
        if let Event::Done { .. } = e {
            l2.append(last_intent.clone().unwrap()).unwrap();
            l2.append(e.clone()).unwrap();
            dup += 2;
        }
    }
    assert!(dup > 40, "{dup} duplicates");
    assert_eq!(semantic(l2.state()), want);
    // and the pure reducer over the raw records agrees
    let recs: Vec<Record> = ledger::read_records(&d2).unwrap();
    assert_eq!(semantic(&reduce(&recs).unwrap()), want);
}

#[test]
fn job_events_of_older_attempts_are_refused() {
    let d = scratch("attempts");
    let _c = Cleanup(d.clone());
    let clk = clock();
    let mut l = new_ledger(&d, as_dyn(&clk), LedgerConfig::default());
    l.append_all(setup_events("T162")).unwrap();
    let life = item_life("T162", 0, 0, EvidenceLevel::IdentityTested, 10);
    l.append_all(life[..4].to_vec()).unwrap(); // created, queued, start, launch
    let j = job_id("T162", 0, 0);
    l.append(Event::Job {
        job: j.clone(),
        attempt: 1,
        ev: JobEvent::Lose {
            reason: "no process after restart".into(),
        },
    })
    .unwrap();
    l.append(Event::Job {
        job: j.clone(),
        attempt: 2,
        ev: JobEvent::Requeue {
            reason: "resume".into(),
        },
    })
    .unwrap();
    // a late record of attempt 1 is refused, never written (audit A m1)
    assert!(l.append(life[3].clone()).is_err());
    assert_eq!(l.state().jobs[&j].attempt, 2);
    // skipping an attempt number is refused
    assert!(
        l.append(Event::Job {
            job: j.clone(),
            attempt: 4,
            ev: JobEvent::Retry { reason: "x".into() }
        })
        .is_err()
    );
    // events for a future attempt are refused
    assert!(
        l.append(Event::Job {
            job: j,
            attempt: 3,
            ev: JobEvent::Launch {
                run_dir: "r".into(),
                cpus: vec![],
                reserved_bytes: 0,
                spec_blake3: digest("r"),
                scope_unit: None,
            }
        })
        .is_err()
    );
}

#[test]
fn changed_input_marks_dependents_stale_never_deletes() {
    let d = scratch("stale");
    let _c = Cleanup(d.clone());
    let clk = clock();
    let mut l = new_ledger(&d, as_dyn(&clk), LedgerConfig::default());
    l.append_all(scenario()).unwrap();
    let items_before = l.state().items.len();
    let jobs_before = l.state().jobs.len();
    assert_eq!(l.state().terminals_total, 3);
    // M1 / C1: a new binary digest -> every item whose result came from a job of that binary becomes stale
    l.append(Event::InputRegistered {
        input: "rr_node".into(),
        role: "binary".into(),
        path: "bin/rr_node".into(),
        bytes: 9_000_001,
        blake3: digest("rr_node v2"),
    })
    .unwrap();
    let st = l.state();
    let stale: Vec<_> = st
        .items
        .iter()
        .filter(|(_, i)| i.phase == ItemPhase::Stale)
        .map(|(k, _)| k.clone())
        .collect();
    assert_eq!(
        stale,
        [0, 1, 2, 3, 6]
            .iter()
            .map(|k| item_id("T162", *k))
            .collect::<Vec<_>>()
    );
    assert!(
        st.jobs
            .values()
            .all(|j| j.stale.as_deref() == Some("rr_node"))
    );
    assert_eq!(
        st.items[&item_id("T162", 0)].stale_from.as_deref(),
        Some("accepted")
    );
    assert_eq!(
        st.items[&item_id("T162", 0)].stale_input.as_deref(),
        Some("rr_node")
    );
    assert_eq!(st.items.len(), items_before);
    assert_eq!(st.jobs.len(), jobs_before);
    assert_eq!(st.inputs["rr_node"].versions, 2);
    // re-registering the same digest is a no-op
    let before = normalised(st.clone());
    l.append(Event::InputRegistered {
        input: "rr_node".into(),
        role: "binary".into(),
        path: "bin/rr_node".into(),
        bytes: 9_000_001,
        blake3: digest("rr_node v2"),
    })
    .unwrap();
    assert_eq!(normalised(l.state().clone()), before);
    // a node input changes -> the node and ALL its items are stale (terminals no longer counted)
    l.append(Event::InputRegistered {
        input: "T162.bank".into(),
        role: "table".into(),
        path: "large:T162.bank.jsonl".into(),
        bytes: 1,
        blake3: digest("bank v2"),
    })
    .unwrap();
    let st = l.state();
    assert_eq!(st.nodes["T162"].state.phase, NodePhase::Stale);
    assert!(
        st.items.values().all(|i| i.phase == ItemPhase::Stale),
        "every item of the node is stale"
    );
    assert_eq!(st.terminals_total, 0);
    assert_eq!(st.nodes["T162"].state.terminals, 0);
    assert_eq!(st.items.len(), items_before);
    // the stale terminal item remembers what it was
    assert_eq!(
        st.items[&item_id("T162", 5)].stale_from.as_deref(),
        Some("terminal")
    );
    // reopen: node first, then items
    assert!(
        l.append(Event::Item {
            item: item_id("T162", 0),
            ev: ItemEvent::Reopen {
                reason: "rerun".into()
            }
        })
        .is_err()
    );
    l.append(Event::Node {
        node: "T162".into(),
        ev: NodeEvent::Reopen {
            reason: "inputs re-registered".into(),
        },
    })
    .unwrap();
    l.append(Event::Item {
        item: item_id("T162", 0),
        ev: ItemEvent::Reopen {
            reason: "rerun".into(),
        },
    })
    .unwrap();
    assert_eq!(l.state().items[&item_id("T162", 0)].phase, ItemPhase::Open);
    assert!(l.state().items[&item_id("T162", 0)].accepted.is_none());
}

#[test]
fn node_verdicts_need_their_evidence() {
    let d = scratch("verdict");
    let _c = Cleanup(d.clone());
    let clk = clock();
    let mut l = new_ledger(&d, as_dyn(&clk), LedgerConfig::default());
    l.append_all(setup_events("T162")).unwrap();
    for k in 0..3 {
        l.append_all(item_life(
            "T162",
            k,
            0,
            if k == 2 {
                EvidenceLevel::GeometryAccepted
            } else {
                EvidenceLevel::IdentityProved
            },
            10,
        ))
        .unwrap();
    }
    let n = |ev| Event::Node {
        node: "T162".into(),
        ev,
    };
    let st = l.state().clone();
    l.append_all(closure_events(&st, "T162", 1, 4)).unwrap();
    assert!(l.append(n(NodeEvent::DeclareClosed)).is_err(), "defect 4");
    l.append(closure_event(&st, "T162", 2, 0, EnvelopeKind::Proved))
        .unwrap();
    l.append(n(NodeEvent::DeclareClosed)).unwrap();
    assert_eq!(l.state().nodes["T162"].state.phase, NodePhase::Closed);
    assert!(
        l.append(n(NodeEvent::DeclareCertified)).is_err(),
        "item 2 only geometry-accepted (FULL asks replay = identity_tested)"
    );
    l.append(Event::Item {
        item: item_id("T162", 2),
        ev: ItemEvent::Upgrade {
            level: EvidenceLevel::IdentityTested,
            evidence: level_evidence(EvidenceLevel::IdentityTested, "2").unwrap(),
        },
    })
    .unwrap();
    l.append(n(NodeEvent::DeclareCertified)).unwrap();
    assert_eq!(l.state().nodes["T162"].state.phase, NodePhase::Certified);
    assert_eq!(l.state().nodes["T162"].state.closure_rounds, 2);
}
