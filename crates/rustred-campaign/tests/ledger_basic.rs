//! Ledger basics: append / reopen, reducer refusal (nothing written), hash chain, torn-tail recovery,
//! corruption detection, write-ahead pending operations, acceptance rules (sealed only, elided refused).
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code
mod ledger_support;

use std::fs;
use std::io::Write;

use ledger_support::*;
use rustred_campaign::ledger::{
    self, AcceptSource, Event, ItemEvent, JobEvent, LedgerConfig, LedgerError,
};
use rustred_campaign::state::{
    AttemptState, ContractAccept, EvidenceLevel, FinishClass, ItemPhase, ReduceError, Rung,
};

fn active_segment(cdir: &std::path::Path) -> std::path::PathBuf {
    let m = ledger::read_manifest(&ledger::ledger_dir(cdir)).unwrap();
    ledger::ledger_dir(cdir).join(m.active.file)
}

#[test]
fn append_reopen_and_chain() {
    let d = scratch("basic");
    let _c = Cleanup(d.clone());
    let clk = clock();
    let mut l = new_ledger(&d, as_dyn(&clk), LedgerConfig::default());
    l.append_all(scenario()).unwrap();
    let seq = l.seq();
    let head = l.head().to_string();
    let digest = l.state().digest();
    drop(l);
    let (l2, rep) = open_ledger(&d, as_dyn(&clk), LedgerConfig::default()).unwrap();
    assert_eq!(rep.recovery, None);
    assert_eq!(rep.replayed, seq);
    assert_eq!((l2.seq(), l2.head()), (seq, head.as_str()));
    assert_eq!(l2.state().digest(), digest, "reopen reproduces the state");
    // chain: every prev is the digest `h` of the previous record; every line carries its own digest
    let bytes = fs::read(active_segment(&d)).unwrap();
    let mut prev = ledger::GENESIS.to_string();
    for line in bytes.split(|&b| b == b'\n').filter(|l| !l.is_empty()) {
        let (r, h) = ledger::decode_record(line).unwrap();
        assert_eq!(r.prev, prev);
        prev = h;
    }
    assert_eq!(prev, head);
    let chain = ledger::verify_chain(&d).unwrap();
    assert_eq!(chain.records, seq);
    assert_eq!(chain.state.digest(), digest);
}

#[test]
fn refused_records_are_not_written() {
    let d = scratch("refuse");
    let _c = Cleanup(d.clone());
    let clk = clock();
    let mut l = new_ledger(&d, as_dyn(&clk), LedgerConfig::default());
    l.append_all(setup_events("T162")).unwrap();
    let before = fs::read(active_segment(&d)).unwrap();
    let seq = l.seq();
    // unknown item
    let e = l
        .append(Event::Item {
            item: "nope".into(),
            ev: ItemEvent::Start { job: None },
        })
        .unwrap_err();
    assert!(
        matches!(e, LedgerError::Refused(ReduceError::Invalid { .. })),
        "{e}"
    );
    // a batch whose LAST record is illegal writes nothing (and the state is rolled back)
    let mut batch = item_life("T162", 0, 0, EvidenceLevel::IdentityTested, 10);
    batch.push(Event::Item {
        item: item_id("T162", 0),
        ev: ItemEvent::Escalate {
            to: Rung(0),
            reason: "x".into(),
        },
    });
    assert!(matches!(
        l.append_all(batch).unwrap_err(),
        LedgerError::Refused(_)
    ));
    assert_eq!(l.seq(), seq);
    assert!(!l.state().items.contains_key(&item_id("T162", 0)));
    assert_eq!(fs::read(active_segment(&d)).unwrap(), before);
    // the handle is still usable
    l.append_all(item_life("T162", 0, 0, EvidenceLevel::IdentityTested, 10))
        .unwrap();
}

#[test]
fn acceptance_only_from_a_sealed_success_and_never_elided() {
    let d = scratch("accept");
    let _c = Cleanup(d.clone());
    let clk = clock();
    let mut l = new_ledger(&d, as_dyn(&clk), LedgerConfig::default());
    l.append_all(setup_events("T162")).unwrap();
    let mut life = item_life("T162", 0, 0, EvidenceLevel::IdentityTested, 10);
    let accept = life.pop().unwrap();
    l.append_all(life).unwrap();
    let Event::Item {
        item,
        ev:
            ItemEvent::Accept {
                rule,
                level,
                record,
                source,
                ..
            },
    } = accept.clone()
    else {
        unreachable!()
    };
    // elided witness loci: refused
    let elided = Event::Item {
        item: item.clone(),
        ev: ItemEvent::Accept {
            rule: rule.clone(),
            level,
            record: record.clone(),
            source: source.clone(),
            elided: true,
            evidence: level_evidence(level, "x"),
            round: None,
        },
    };
    assert!(matches!(l.append(elided), Err(LedgerError::Refused(_))));
    // wrong table digest: refused
    let AcceptSource::Job { job, attempt, .. } = source.clone() else {
        unreachable!()
    };
    let wrong = Event::Item {
        item: item.clone(),
        ev: ItemEvent::Accept {
            rule: rule.clone(),
            level,
            record: record.clone(),
            source: AcceptSource::Job {
                job: job.clone(),
                attempt,
                table_blake3: digest("other"),
            },
            elided: false,
            evidence: level_evidence(level, "x"),
            round: None,
        },
    };
    assert!(matches!(l.append(wrong), Err(LedgerError::Refused(_))));
    l.append(accept.clone()).unwrap();
    // repeat of the same acceptance: no-op; a different rule for the same item: refused
    l.append(accept).unwrap();
    assert_eq!(l.state().rules.accepted, 1);
    let other = Event::Item {
        item: item.clone(),
        ev: ItemEvent::Accept {
            rule: "other".into(),
            level,
            record,
            source,
            elided: false,
            evidence: level_evidence(level, "x"),
            round: None,
        },
    };
    assert!(matches!(l.append(other), Err(LedgerError::Refused(_))));
    // a success class without a sealed output is refused by the attempt machine
    l.append_all(vec![
        Event::ItemCreated {
            item: "i2".into(),
            node: "T162".into(),
            kind: "face".into(),
            rung: Rung(0),
            parent: None,
            inputs: vec![],
            label: "x".into(),
            weight: 1,
        },
        Event::JobQueued {
            job: "j2".into(),
            item: "i2".into(),
            rung: Rung(0),
            kind: "generate".into(),
            spec_blake3: digest("j2"),
            binary: "rr_node".into(),
            reserved_bytes: 1,
            inputs: vec![],
        },
        Event::Job {
            job: "j2".into(),
            attempt: 1,
            ev: JobEvent::Launch {
                run_dir: "r".into(),
                cpus: vec![],
                reserved_bytes: 1,
                spec_blake3: digest("j2"),
                scope_unit: None,
            },
        },
    ])
    .unwrap();
    assert!(matches!(
        l.append(Event::Job {
            job: "j2".into(),
            attempt: 1,
            ev: finished_with(
                "j2",
                1,
                FinishClass::RuleFound,
                ContractAccept::Canonical,
                None,
                5
            )
        }),
        Err(LedgerError::Refused(ReduceError::Illegal { .. }))
    ));
    l.append(Event::Job {
        job: "j2".into(),
        attempt: 1,
        ev: finished(FinishClass::KilledTime, None, 5),
    })
    .unwrap();
    assert!(matches!(
        l.state().jobs["j2"].state,
        AttemptState::BudgetStopped(FinishClass::KilledTime)
    ));
    assert_eq!(
        l.state().items[&item].phase,
        ItemPhase::Accepted(EvidenceLevel::IdentityTested)
    );
}

#[test]
fn torn_tail_is_truncated_saved_and_recorded() {
    let d = scratch("torn");
    let _c = Cleanup(d.clone());
    let clk = clock();
    let mut l = new_ledger(&d, as_dyn(&clk), LedgerConfig::default());
    l.append_all(scenario()).unwrap();
    let (seq, head) = (l.seq(), l.head().to_string());
    drop(l);
    let seg = active_segment(&d);
    let good_len = fs::metadata(&seg).unwrap().len();
    let torn = br#"{"schema":"rustred.campaign.ledger.v1","seq":999,"utc":"2026-11-13T"#;
    fs::OpenOptions::new()
        .append(true)
        .open(&seg)
        .unwrap()
        .write_all(torn)
        .unwrap();
    let (l2, rep) = open_ledger(&d, as_dyn(&clk), LedgerConfig::default()).unwrap();
    let rec = rep.recovery.expect("recovery reported");
    assert_eq!(rec.last_good_seq, seq);
    assert_eq!(rec.truncated_bytes, torn.len() as u64);
    let saved = ledger::ledger_dir(&d).join(rec.saved_as.unwrap());
    assert_eq!(fs::read(saved).unwrap(), torn);
    assert_eq!(l2.seq(), seq + 1, "recovery record appended");
    assert_eq!(l2.state().torn_tail_recoveries, 1);
    let (last, _) = ledger::decode_record(
        fs::read(&seg).unwrap()[good_len as usize..]
            .strip_suffix(b"\n")
            .unwrap(),
    )
    .unwrap();
    assert_eq!(last.prev, head);
    assert!(matches!(last.event, Event::Recovery { .. }));
    drop(l2);
    // a garbage complete line at the tail (power-loss zeros) is also a torn tail
    fs::OpenOptions::new()
        .append(true)
        .open(&seg)
        .unwrap()
        .write_all(&[0u8; 37])
        .unwrap();
    fs::OpenOptions::new()
        .append(true)
        .open(&seg)
        .unwrap()
        .write_all(b"\n")
        .unwrap();
    let (_l3, rep) = open_ledger(&d, as_dyn(&clk), LedgerConfig::default()).unwrap();
    assert_eq!(rep.recovery.unwrap().truncated_bytes, 38);
    assert!(ledger::verify_chain(&d).is_ok());
}

#[test]
fn corruption_followed_by_valid_records_refuses() {
    let d = scratch("corrupt");
    let _c = Cleanup(d.clone());
    let clk = clock();
    let mut l = new_ledger(&d, as_dyn(&clk), LedgerConfig::default());
    l.append_all(scenario()).unwrap();
    drop(l);
    let seg = active_segment(&d);
    let bytes = fs::read(&seg).unwrap();
    // edit one record in the middle (change a digit of a cpu figure without touching the length)
    let text = String::from_utf8(bytes.clone()).unwrap();
    let pos = text.find("\"cpu_ms\":2000").expect("record present");
    let mut edited = bytes.clone();
    edited[pos + 9] = b'9';
    fs::write(&seg, &edited).unwrap();
    match open_ledger(&d, as_dyn(&clk), LedgerConfig::default()) {
        Err(LedgerError::Corrupt { why, .. }) => {
            assert!(why.contains("digest mismatch"), "{why}")
        }
        other => panic!("expected Corrupt, got {other:?}"),
    }
    assert!(matches!(
        ledger::verify_chain(&d),
        Err(LedgerError::Corrupt { .. })
    ));
    // deleting a record in the middle is detected too (sequence gap / chain)
    let lines: Vec<&[u8]> = bytes
        .split(|&b| b == b'\n')
        .filter(|l| !l.is_empty())
        .collect();
    let mut cut = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        if i != 10 {
            cut.extend_from_slice(l);
            cut.push(b'\n');
        }
    }
    fs::write(&seg, &cut).unwrap();
    assert!(matches!(
        open_ledger(&d, as_dyn(&clk), LedgerConfig::default()),
        Err(LedgerError::Corrupt { .. })
    ));
}

#[test]
fn pending_operations_survive_reopen() {
    let d = scratch("pending");
    let _c = Cleanup(d.clone());
    let clk = clock();
    let mut l = new_ledger(&d, as_dyn(&clk), LedgerConfig::default());
    l.append_all(setup_events("T162")).unwrap();
    l.append(Event::Intent {
        op: "op-1".into(),
        action: "write_keyset".into(),
        target: "large:keysets/a.keys".into(),
    })
    .unwrap();
    // a crash here: the intent is pending after reopen, so the supervisor redoes the (idempotent,
    // content-addressed) side effect and then writes `done`.
    drop(l);
    let (mut l, _) = open_ledger(&d, as_dyn(&clk), LedgerConfig::default()).unwrap();
    assert!(l.state().pending_ops.contains_key("op-1"));
    l.append(Event::Done {
        op: "op-1".into(),
        result: Some(fref("large:keysets/a.keys")),
    })
    .unwrap();
    assert!(l.state().pending_ops.is_empty());
    assert!(matches!(
        l.append(Event::Done {
            op: "op-unknown".into(),
            result: None
        }),
        Err(LedgerError::Refused(_))
    ));
}

#[test]
fn lazy_records_are_written_at_once() {
    let d = scratch("lazy");
    let _c = Cleanup(d.clone());
    let clk = clock();
    let mut l = new_ledger(&d, as_dyn(&clk), LedgerConfig::default());
    l.append(Event::Note {
        text: "lazy".into(),
    })
    .unwrap();
    // visible to another reader immediately (written with one write; fsync later)
    let recs = ledger::read_records(&d).unwrap();
    assert!(matches!(recs.last().unwrap().event, Event::Note { .. }));
    assert_eq!(
        Event::Note {
            text: String::new()
        }
        .durability(),
        ledger::Durability::Lazy
    );
    assert_eq!(
        Event::Item {
            item: "x".into(),
            ev: ItemEvent::Reject {
                reason: "r".into(),
                disproof: None
            }
        }
        .durability(),
        ledger::Durability::Sync
    );
    l.sync().unwrap();
}
