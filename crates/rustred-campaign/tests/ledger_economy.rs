//! R31 / R33 / R35 through the ledger: FULL first, BOXED only as a fallback to the SPEC box, OPEN otherwise;
//! rule retirement needs a coverage-preservation proof on FULL nodes (a box closure pass only on BOXED ones);
//! supersession is explicit (never deletion) and the reducer reports economy before / after.
mod ledger_support;

use ledger_support::*;
use rustred_campaign::ledger::{
    Event, ItemEvent, Ledger, LedgerConfig, LedgerError, RetirementProof, RuleEconomy,
};
use rustred_campaign::progress;
use rustred_campaign::state::{
    Blockers, ClosureTarget, EvidenceLevel, ItemPhase, NodeEvent, NodePhase, ReduceError,
};

fn node(ev: NodeEvent) -> Event {
    Event::Node {
        node: "T162".into(),
        ev,
    }
}

fn ledger_with_accepted(tag: &str, n: usize) -> (Ledger, Cleanup) {
    let d = scratch(tag);
    let c = Cleanup(d.clone());
    let clk = clock();
    let mut l = new_ledger(&d, as_dyn(&clk), LedgerConfig::default());
    l.append_all(setup_events("T162")).unwrap();
    for k in 0..n {
        l.append_all(item_life("T162", k, 0, EvidenceLevel::IdentityProved, 100))
            .unwrap();
        l.append(Event::RuleEconomy {
            item: item_id("T162", k),
            economy: RuleEconomy {
                cases: 1,
                rules: 1,
                rhs_terms: 2000,
                monomials: 9000,
                coefficient_degree: 10,
                bytes: 50_000,
            },
        })
        .unwrap();
    }
    (l, c)
}

fn retire(k: usize, proof: RetirementProof, by: Option<usize>) -> Event {
    Event::Item {
        item: item_id("T162", k),
        ev: ItemEvent::Retire {
            rule: format!("T162:R:campaign:{k}"),
            proof,
            superseded_by: by.map(|b| item_id("T162", b)),
        },
    }
}

#[test]
fn full_node_retirement_needs_a_coverage_preservation_proof() {
    let (mut l, _c) = ledger_with_accepted("econ-full", 4);
    assert_eq!(l.state().nodes["T162"].state.target, ClosureTarget::Full);
    // a box closure pass never prunes a FULL node
    let e = l
        .append(retire(
            0,
            RetirementProof::BoxClosurePass {
                closure: fref("large:closure/1.keys"),
            },
            None,
        ))
        .unwrap_err();
    assert!(
        matches!(e, LedgerError::Refused(ReduceError::Illegal { .. })),
        "{e}"
    );
    // replacement must name the superseding rule
    assert!(
        l.append(retire(
            0,
            RetirementProof::ReplacedBy {
                proof: fref("p/0.json")
            },
            None
        ))
        .is_err()
    );
    assert!(
        l.append(retire(
            0,
            RetirementProof::ReplacedBy {
                proof: fref("p/0.json")
            },
            Some(0)
        ))
        .is_err(),
        "not by itself"
    );
    // coverage-preserving kinds are fine
    l.append(retire(
        0,
        RetirementProof::FullyShadowed {
            proof: fref("p/shadow-0.json"),
        },
        None,
    ))
    .unwrap();
    l.append(retire(
        1,
        RetirementProof::ReplacedBy {
            proof: fref("p/contain-1-in-2.json"),
        },
        Some(2),
    ))
    .unwrap();
    l.append(retire(
        3,
        RetirementProof::EmptyDomain {
            proof: fref("p/empty-3.json"),
        },
        None,
    ))
    .unwrap();
    // superseding a retired item is impossible; retiring twice is a repeat
    assert!(
        l.append(retire(
            2,
            RetirementProof::ReplacedBy {
                proof: fref("p/x.json")
            },
            Some(1)
        ))
        .is_err()
    );
    l.append(retire(
        3,
        RetirementProof::EmptyDomain {
            proof: fref("p/empty-3.json"),
        },
        None,
    ))
    .unwrap();
    let st = l.state();
    assert_eq!(st.items[&item_id("T162", 1)].phase, ItemPhase::Retired);
    assert_eq!(
        st.items[&item_id("T162", 1)]
            .retired
            .as_ref()
            .unwrap()
            .superseded_by
            .as_deref(),
        Some(item_id("T162", 2).as_str())
    );
    assert!(
        st.items[&item_id("T162", 1)].accepted.is_some(),
        "the record is kept, never deleted"
    );
    assert_eq!(st.items.len(), 4);
    let e = st.node_economy("T162");
    assert_eq!((e.rules_before, e.rules_after, e.superseded), (4, 1, 1));
    assert_eq!((e.before.rhs_terms, e.after.rhs_terms), (8000, 2000));
    assert_eq!(st.rules.retired, 3);
    // the progress files show it
    let set = progress::derive(st, l.head(), &Default::default(), "t");
    assert_eq!(set["nodes/T162.json"]["economy"]["rules_after"], 1);
    assert_eq!(set["campaign.json"]["economy"]["after"]["rhs_terms"], 2000);
}

#[test]
fn boxed_only_after_the_full_attempt_and_only_with_the_spec_box() {
    let (mut l, _c) = ledger_with_accepted("econ-box", 2);
    let blk = Blockers {
        faces: 2,
        loci: 1,
        keys: 150,
        list: Some(fref("large:blockers/T162-full.keys")),
    };
    assert!(
        l.append(node(NodeEvent::FallBackToBox {
            box_id: "T162.box".into()
        }))
        .is_err(),
        "full attempt first"
    );
    l.append(node(NodeEvent::AttemptEnded {
        blockers: blk.clone(),
        reason: "full ladder ended within budgets".into(),
    }))
    .unwrap();
    assert!(
        l.append(node(NodeEvent::FallBackToBox {
            box_id: "T162.family".into()
        }))
        .is_err(),
        "not the spec box"
    );
    assert!(
        l.append(node(NodeEvent::DeclareOpen { reason: "x".into() }))
            .is_err(),
        "the spec box must be tried"
    );
    l.append(node(NodeEvent::FallBackToBox {
        box_id: "T162.box".into(),
    }))
    .unwrap();
    l.append(node(NodeEvent::ClosureResult {
        round: 1,
        defect_keys: 0,
        closure_keys: 154_783,
        rules_used: Some(1),
        keyset: None,
    }))
    .unwrap();
    // box pruning is legal now
    l.append(retire(
        0,
        RetirementProof::BoxClosurePass {
            closure: fref("large:closure/T162-1.keys"),
        },
        None,
    ))
    .unwrap();
    l.append(node(NodeEvent::DeclareClosed)).unwrap();
    l.append(node(NodeEvent::DeclareCertified)).unwrap();
    let n = &l.state().nodes["T162"];
    assert_eq!(
        (n.state.phase, n.state.closure_kind()),
        (NodePhase::Certified, "boxed")
    );
    assert_eq!(
        n.state.full_blockers.as_ref(),
        Some(&blk),
        "what prevented FULL is kept for a later run"
    );
    let set = progress::derive(l.state(), l.head(), &Default::default(), "t");
    assert_eq!(set["nodes/T162.json"]["closure_kind"], "boxed");
    assert_eq!(set["nodes/T162.json"]["certified"], true);
    assert_eq!(set["nodes/T162.json"]["blockers"]["full"]["keys"], 150);
    assert_eq!(set["campaign.json"]["nodes"]["by_closure"]["boxed"], 1);
}

#[test]
fn node_without_spec_box_ends_open_never_boxed() {
    let d = scratch("econ-open");
    let _c = Cleanup(d.clone());
    let clk = clock();
    let mut l = new_ledger(&d, as_dyn(&clk), LedgerConfig::default());
    let mut setup = setup_events("T162");
    for e in setup.iter_mut() {
        if let Event::NodeRegistered { spec_box, .. } = e {
            *spec_box = None;
        }
    }
    l.append_all(setup).unwrap();
    l.append(node(NodeEvent::AttemptEnded {
        blockers: Blockers {
            keys: 9,
            ..Default::default()
        },
        reason: "budget".into(),
    }))
    .unwrap();
    assert!(
        l.append(node(NodeEvent::FallBackToBox {
            box_id: "T162.box".into()
        }))
        .is_err(),
        "BOXED without a spec box"
    );
    l.append(node(NodeEvent::DeclareOpen {
        reason: "full closure not reached".into(),
    }))
    .unwrap();
    assert_eq!(l.state().nodes["T162"].state.closure_kind(), "open");
    // a spec box must be an input of role "box"
    let mut l2 = new_ledger(&scratch("econ-role"), as_dyn(&clk), LedgerConfig::default());
    let mut setup = setup_events("T178");
    for e in setup.iter_mut() {
        if let Event::NodeRegistered { spec_box, .. } = e {
            *spec_box = Some("T178.family".into());
        }
    }
    assert!(matches!(l2.append_all(setup), Err(LedgerError::Refused(_))));
}
