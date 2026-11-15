//! lane symfix (audit r2-sym F5): the ledger does not depend on the work source calling `route_settled`: a job whose
//! contract row accepts `supplied_relations` may have its sealed table PRODUCED (the candidates are recorded), but an
//! item ACCEPT (a direct credit) from it is REFUSED whatever the source wrote: supplied relations become evidence only
//! through the node import's load-time verification (interfaces/import.md 13.3). Control: the same life with accept
//! `canonical` is credited. Pure ledger test (no process).
mod ledger_support;

use ledger_support::*;
use rustred_campaign::ledger::{Event, JobEvent, LedgerConfig, LedgerError, OutcomeBinding};
use rustred_campaign::state::{ContractAccept, EvidenceLevel, FinishClass, ItemPhase};

const N: &str = "T162";

fn refused(r: Result<u64, LedgerError>) -> bool {
    matches!(r, Err(LedgerError::Refused(_)))
}

#[test]
fn a_supplied_relations_attempt_is_never_credited_directly() {
    for class in [FinishClass::Complete, FinishClass::RuleFound] {
        let d = scratch(&format!("symfix-f5-{class:?}"));
        let _c = Cleanup(d.clone());
        let clk = clock();
        let mut l = new_ledger(&d, as_dyn(&clk), LedgerConfig::default());
        l.append_all(setup_events(N)).unwrap();
        let mut life = item_life(N, 0, 0, EvidenceLevel::GeometryAccepted, 10);
        let j = job_id(N, 0, 0);
        // life[5] = the attempt's finish: the symmetry-composite rows (33 complete / 34 rule found) accept
        // `supplied_relations` from a sealed table
        let mut fin = finished_with(
            &j,
            1,
            class,
            ContractAccept::SuppliedRelations,
            Some(table_ref(&j, 1)),
            10,
        );
        // a records-accepting finish carries its OUTCOME binding (the helper sets it for canonical rows only)
        if let JobEvent::Finished {
            outcome, binding, ..
        } = &mut fin
        {
            *outcome = Some(fref(&format!("large:jobs/{j}.a1/OUTCOME.json")));
            *binding = Some(OutcomeBinding {
                job: j.clone(),
                attempt: 1,
                spec_blake3: digest(&j),
            });
        }
        life[5] = Event::Job {
            job: j.clone(),
            attempt: 1,
            ev: fin,
        };
        let accept = life.pop().expect("the accept event");
        // finish, produce and the accepted-record write are recorded (the candidates exist)
        l.append_all(life).unwrap();
        // the direct credit is refused
        assert!(
            refused(l.append(accept)),
            "{class:?}: an item accept from a supplied_relations attempt was credited"
        );
        assert!(!matches!(
            l.state().items[&item_id(N, 0)].phase,
            ItemPhase::Accepted(_)
        ));
    }
    // control: accept canonical (row 1 / 2) is credited exactly as before
    let d = scratch("symfix-f5-control");
    let _c = Cleanup(d.clone());
    let clk = clock();
    let mut l = new_ledger(&d, as_dyn(&clk), LedgerConfig::default());
    l.append_all(setup_events(N)).unwrap();
    l.append_all(item_life(N, 0, 0, EvidenceLevel::GeometryAccepted, 10))
        .unwrap();
    assert!(matches!(
        l.state().items[&item_id(N, 0)].phase,
        ItemPhase::Accepted(_)
    ));
}
