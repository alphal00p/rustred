//! Regression tests of the two ledger audits (notes/implementer.audit-ledger-A.md, -B.md): one test per
//! finding id, each porting the auditor's reproduction and asserting the FIXED behaviour (they fail on e371e1f5).
//! Plus the R40 / R41 rulings applied to the records (U-*, P-*).
mod ledger_support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;

use ledger_support::*;
use rustred_campaign::clock::SystemClock;
use rustred_campaign::ledger::{
    self, AcceptSource, CampaignMode, ControlEvent, ControlTarget, ControlVerb, EnvelopeKind,
    Event, Genesis, ItemEvent, JobEvent, Ledger, LedgerConfig, LedgerError, LevelEvidence,
    OpenOptions, RetirementProof, RuleEconomy,
};
use rustred_campaign::lock::{self, DeadReason, LockOutcome};
use rustred_campaign::progress::{self, ProgressWriter, Resources};
use rustred_campaign::state::{
    AttemptEndCause, AttemptState, Blockers, ClosureTarget, ContractAccept, EvidenceLevel,
    FinishClass, ItemPhase, NodeEvent, NodePhase, Rung,
};

const N: &str = "T162";

fn cfg() -> LedgerConfig {
    LedgerConfig::default()
}

fn node(ev: NodeEvent) -> Event {
    Event::Node { node: N.into(), ev }
}

fn item(k: usize, ev: ItemEvent) -> Event {
    Event::Item {
        item: item_id(N, k),
        ev,
    }
}

fn refused(r: Result<u64, LedgerError>) -> bool {
    matches!(r, Err(LedgerError::Refused(_)))
}

fn seg_path(d: &std::path::Path) -> std::path::PathBuf {
    let m = ledger::read_manifest(&ledger::ledger_dir(d)).unwrap();
    ledger::ledger_dir(d).join(m.active.file)
}

/// A ledger with the node set up and items 0..n accepted at `level`.
fn with_items(tag: &str, n: usize, level: EvidenceLevel) -> (Ledger, Cleanup, std::path::PathBuf) {
    let d = scratch(tag);
    let c = Cleanup(d.clone());
    let clk = clock();
    let mut l = new_ledger(&d, as_dyn(&clk), cfg());
    l.append_all(setup_events(N)).unwrap();
    for k in 0..n {
        l.append_all(item_life(N, k, 0, level, 10)).unwrap();
    }
    (l, c, d)
}

fn close(l: &mut Ledger, round: u32) {
    let st = l.state().clone();
    l.append_all(closure_events(&st, N, round, 0)).unwrap();
    l.append(node(NodeEvent::DeclareClosed)).unwrap();
    assert_eq!(l.state().nodes[N].state.phase, NodePhase::Closed);
}

fn reopen_and_start(l: &mut Ledger, k: usize) {
    l.append(item(
        k,
        ItemEvent::Reopen {
            reason: "rerun".into(),
        },
    ))
    .unwrap();
    l.append(item(k, ItemEvent::Start { job: None })).unwrap();
}

/// A fresh job for item `k` under the current inputs: queued, launched, finished (rule found, sealed).
fn new_job(l: &mut Ledger, k: usize, job: &str) -> ledger::FileRef {
    let table = table_ref(job, 1);
    l.append_all(vec![
        job_queued(job, &item_id(N, k), 0, "generate"),
        Event::Job {
            job: job.into(),
            attempt: 1,
            ev: JobEvent::Launch {
                run_dir: format!("large:jobs/{job}.a1"),
                cpus: vec![1],
                reserved_bytes: 1,
            },
        },
        Event::Job {
            job: job.into(),
            attempt: 1,
            ev: finished_with(
                job,
                1,
                FinishClass::RuleFound,
                ContractAccept::Canonical,
                Some(table.clone()),
                10,
            ),
        },
    ])
    .unwrap();
    table
}

fn accept_from(k: usize, job: &str, level: EvidenceLevel) -> Event {
    item(
        k,
        ItemEvent::Accept {
            rule: rule_id(N, k),
            level,
            record: record_ref(N, k),
            source: AcceptSource::Job {
                job: job.into(),
                attempt: 1,
                table_blake3: table_ref(job, 1).blake3,
            },
            elided: false,
            evidence: level_evidence(level, "x"),
            round: None,
        },
    )
}

// ------------------------------------------------------------------------------------------------------------
// Audit A: CRITICAL
// ------------------------------------------------------------------------------------------------------------

#[test]
fn a_c1_stale_results_are_never_reaccepted_and_jobs_carry_input_identity() {
    let (mut l, _c, _d) = with_items("c1", 0, EvidenceLevel::GeometryAccepted);
    let life = item_life(N, 0, 0, EvidenceLevel::GeometryAccepted, 10);
    l.append_all(life.clone()).unwrap();
    // a second item only PRODUCED (not accepted) when the binary changes
    l.append_all(item_until_produced(N, 1, 0, 10)).unwrap();
    let job0 = job_id(N, 0, 0);
    assert_eq!(
        l.state().jobs[&job0].inputs.get("rr_node"),
        Some(&digest("rr_node v1")),
        "the job is stamped with its inputs"
    );
    l.append(input("rr_node", "binary", "rr_node v2")).unwrap();
    let st = l.state();
    assert_eq!(st.items[&item_id(N, 0)].phase, ItemPhase::Stale);
    assert_eq!(
        st.items[&item_id(N, 1)].phase,
        ItemPhase::Stale,
        "a produced-unaccepted result of the old binary is stale too"
    );
    assert_eq!(st.jobs[&job0].stale.as_deref(), Some("rr_node"));
    // the auditor's sequence: reopen, start, replay the OLD produce + accept -> refused, nothing written
    reopen_and_start(&mut l, 0);
    let len = fs::metadata(seg_path(&_d)).unwrap().len();
    assert!(
        refused(l.append(life[6].clone())),
        "produce from the old job"
    );
    assert!(
        refused(l.append(life[9].clone())),
        "accept from the old job"
    );
    assert_eq!(fs::metadata(seg_path(&_d)).unwrap().len(), len);
    // only a new attempt under the new inputs can be accepted
    let t = new_job(&mut l, 0, "j-new");
    l.append(item(
        0,
        ItemEvent::Produce {
            job: "j-new".into(),
            attempt: 1,
            table: t,
        },
    ))
    .unwrap();
    l.append(accept_from(0, "j-new", EvidenceLevel::GeometryAccepted))
        .unwrap();
    assert!(matches!(
        l.state().items[&item_id(N, 0)].phase,
        ItemPhase::Accepted(_)
    ));
}

#[test]
fn a_c1_children_of_a_stale_result_become_stale_and_old_generation_jobs_are_refused() {
    let (mut l, _c, _d) = with_items("c1b", 1, EvidenceLevel::GeometryAccepted);
    // item 1 child of item 0 (spawned from its result)
    l.append(item_created(N, 1, 1, Some(item_id(N, 0)), "locus"))
        .unwrap();
    l.append(input("rr_node", "binary", "rr_node v2")).unwrap();
    assert_eq!(l.state().items[&item_id(N, 1)].phase, ItemPhase::Stale);
    // generation: a job queued before a requeue cannot feed the reopened item
    l.append_all(item_until_produced(N, 2, 0, 10)[..5].to_vec())
        .unwrap();
    let j = job_id(N, 2, 0);
    l.append(item(
        2,
        ItemEvent::Requeue {
            reason: "interrupted".into(),
        },
    ))
    .unwrap();
    l.append(item(2, ItemEvent::Start { job: None })).unwrap();
    l.append(Event::Job {
        job: j.clone(),
        attempt: 1,
        ev: finished_with(
            &j,
            1,
            FinishClass::RuleFound,
            ContractAccept::Canonical,
            Some(table_ref(&j, 1)),
            5,
        ),
    })
    .unwrap();
    assert!(refused(l.append(item(
        2,
        ItemEvent::Produce {
            job: j.clone(),
            attempt: 1,
            table: table_ref(&j, 1),
        }
    ))));
}

#[test]
fn a_c2_node_verdict_is_derived_and_withdrawn_automatically() {
    let (mut l, _c, _d) = with_items("c2", 1, EvidenceLevel::GeometryAccepted);
    close(&mut l, 1);
    // reject of the accepted item: the node goes back to running (node_reopened)
    l.append(item(
        0,
        ItemEvent::Reject {
            reason: "identity test failed".into(),
            disproof: Some(fref("large:disproof/0.json")),
        },
    ))
    .unwrap();
    let n = &l.state().nodes[N];
    assert_eq!(n.state.phase, NodePhase::Running);
    assert!(n.state.reopen_cause.as_ref().unwrap().contains("rejected"));
    assert!(l.state().recent.iter().any(|e| e.kind == "node_reopened"));
    assert!(refused(l.append(node(NodeEvent::DeclareCertified))));
    // a new item on a closed node reopens it
    let (mut l, _c2, _d2) = with_items("c2b", 1, EvidenceLevel::IdentityTested);
    close(&mut l, 1);
    l.append(item_created(N, 5, 0, None, "face")).unwrap();
    assert_eq!(l.state().nodes[N].state.phase, NodePhase::Running);
    // a closure result of an older rule set is refused; a stale binding cannot close
    let (mut l, _c3, _d3) = with_items("c2c", 1, EvidenceLevel::IdentityTested);
    let old = l.state().clone();
    l.append(assembled(N, 1)).unwrap();
    l.append(closure_event(&old, N, 1, 0, EnvelopeKind::Proved))
        .unwrap();
    l.append_all(item_life(N, 1, 0, EvidenceLevel::IdentityTested, 10))
        .unwrap();
    assert!(
        refused(l.append(node(NodeEvent::DeclareClosed))),
        "the closure predates an acceptance"
    );
    assert!(
        refused(l.append(closure_event(&old, N, 2, 0, EnvelopeKind::Proved))),
        "computed on an older rule set"
    );
}

#[test]
fn a_c2_replacement_rejected_after_retirement_reopens_the_full_node() {
    let (mut l, _c, _d) = with_items("c2n", 2, EvidenceLevel::IdentityTested);
    l.append(item(
        0,
        ItemEvent::Retire {
            rule: rule_id(N, 0),
            proof: RetirementProof::ReplacedBy { proof: fref("p") },
            superseded_by: Some(item_id(N, 1)),
        },
    ))
    .unwrap();
    close(&mut l, 1);
    l.append(item(
        1,
        ItemEvent::Reject {
            reason: "proof failed".into(),
            disproof: None,
        },
    ))
    .unwrap();
    assert_eq!(l.state().nodes[N].state.phase, NodePhase::Running);
    assert_eq!(l.state().nodes[N].state.closure_kind(), "open");
    // the replaced rule can be reinstated (coverage back)
    l.append(item(
        0,
        ItemEvent::Reinstate {
            reason: "replacement withdrawn".into(),
        },
    ))
    .unwrap();
    assert!(matches!(
        l.state().items[&item_id(N, 0)].phase,
        ItemPhase::Accepted(EvidenceLevel::IdentityTested)
    ));
}

#[test]
fn a_c3_acceptance_follows_the_contract_row() {
    let (mut l, _c, _d) = with_items("c3", 0, EvidenceLevel::GeometryAccepted);
    let mut life = item_life(N, 0, 0, EvidenceLevel::GeometryAccepted, 10);
    let j = job_id(N, 0, 0);
    let verdict = fref("large:jobs/x/verdict.json");
    // checker decided FAIL (row 7): accept canonical is refused by the attempt machine
    life[5] = Event::Job {
        job: j.clone(),
        attempt: 1,
        ev: finished_with(
            &j,
            1,
            FinishClass::DecidedNegative,
            ContractAccept::Canonical,
            Some(verdict.clone()),
            10,
        ),
    };
    l.append_all(life[..5].to_vec()).unwrap();
    assert!(refused(l.append(life[5].clone())));
    // with accept nothing (its row) the attempt settles, but nothing can be produced from it
    l.append(Event::Job {
        job: j.clone(),
        attempt: 1,
        ev: finished_with(
            &j,
            1,
            FinishClass::DecidedNegative,
            ContractAccept::Nothing,
            Some(verdict.clone()),
            10,
        ),
    })
    .unwrap();
    assert!(refused(l.append(item(
        0,
        ItemEvent::Produce {
            job: j,
            attempt: 1,
            table: verdict,
        }
    ))));
    // spill (row 8: sealed partial table, accept committed rules) IS acceptable
    let mut life = item_life(N, 1, 0, EvidenceLevel::GeometryAccepted, 10);
    let j1 = job_id(N, 1, 0);
    life[5] = Event::Job {
        job: j1.clone(),
        attempt: 1,
        ev: finished_with(
            &j1,
            1,
            FinishClass::Spill,
            ContractAccept::CommittedRules,
            Some(table_ref(&j1, 1)),
            10,
        ),
    };
    l.append_all(life).unwrap();
    assert!(matches!(
        l.state().jobs[&j1].state,
        AttemptState::BudgetStopped(FinishClass::Spill)
    ));
    assert!(matches!(
        l.state().items[&item_id(N, 1)].phase,
        ItemPhase::Accepted(_)
    ));
    // records-accepting finish without the OUTCOME binding is refused
    l.append_all(item_until_produced(N, 2, 0, 10)[..5].to_vec())
        .unwrap();
    let j2 = job_id(N, 2, 0);
    let mut f = finished_with(
        &j2,
        1,
        FinishClass::RuleFound,
        ContractAccept::Canonical,
        Some(table_ref(&j2, 1)),
        10,
    );
    if let JobEvent::Finished { binding, .. } = &mut f {
        binding.as_mut().unwrap().spec_blake3 = digest("another spec");
    }
    assert!(refused(l.append(Event::Job {
        job: j2,
        attempt: 1,
        ev: f
    })));
}

fn last_line_range(bytes: &[u8]) -> (usize, usize) {
    let end = bytes.len() - 1;
    let start = bytes[..end]
        .iter()
        .rposition(|&b| b == b'\n')
        .map_or(0, |p| p + 1);
    (start, end)
}

#[test]
fn a_c4_acknowledged_last_record_is_protected() {
    // (a1) a flipped byte in the last acknowledged line: corruption, not a torn tail
    let (l, _c, d) = with_items("c4", 1, EvidenceLevel::GeometryAccepted);
    let seq = l.seq();
    drop(l);
    let seg = seg_path(&d);
    let orig = fs::read(&seg).unwrap();
    let mut b = orig.clone();
    let (s, _) = last_line_range(&b);
    b[s] = b'X';
    fs::write(&seg, &b).unwrap();
    let clk = clock();
    assert!(matches!(
        open_ledger(&d, as_dyn(&clk), cfg()),
        Err(LedgerError::Corrupt { .. })
    ));
    assert_eq!(fs::read(&seg).unwrap(), b, "nothing truncated");
    // (a2) an edit inside a string keeps the JSON valid but breaks the line digest
    let txt = String::from_utf8(orig.clone()).unwrap();
    fs::write(
        &seg,
        txt.replacen("T162:R:campaign:0", "T162:R:campaign:9", 1),
    )
    .unwrap();
    assert!(matches!(
        open_ledger(&d, as_dyn(&clk), cfg()),
        Err(LedgerError::Corrupt { .. })
    ));
    assert!(ledger::verify_chain(&d).is_err());
    assert!(ledger::open_read_only(&d).is_err());
    // acknowledged records missing (the last line cut away): the commit mark names them -> corruption
    let (s, _) = last_line_range(&orig);
    fs::write(&seg, &orig[..s]).unwrap();
    match open_ledger(&d, as_dyn(&clk), cfg()) {
        Err(LedgerError::Corrupt { why, .. }) => assert!(why.contains("missing"), "{why}"),
        other => panic!("{other:?}"),
    }
    // the operator override truncates after a named seq, saves the bytes, records it as forced
    fs::write(&seg, &b).unwrap();
    let (l, rep) = open_ledger_with(
        &d,
        as_dyn(&clk),
        cfg(),
        Arc::new(ledger::RealOps),
        OpenOptions {
            accept_truncation_to: Some(seq - 1),
        },
    )
    .unwrap();
    let r = rep.recovery.unwrap();
    assert!(r.forced && r.last_good_seq == seq - 1);
    let saved = fs::read(ledger::ledger_dir(&d).join(r.saved_as.unwrap())).unwrap();
    assert_eq!(saved, &b[s..]);
    assert_eq!(l.state().forced_recoveries, 1);
    assert!(matches!(
        l.state().items[&item_id(N, 0)].phase,
        ItemPhase::Produced
    ));
}

// ------------------------------------------------------------------------------------------------------------
// Audit A: MAJOR
// ------------------------------------------------------------------------------------------------------------

#[test]
fn a_m1_failed_fsync_poisons_and_nothing_is_acknowledged_after_it() {
    let d = scratch("m1");
    let _c = Cleanup(d.clone());
    let clk = clock();
    let ops = Ops::new();
    let mut l =
        new_ledger_with(&d, as_dyn(&clk), cfg(), Arc::new(RecordingOps(ops.clone()))).unwrap();
    l.append_all(setup_events(N)).unwrap();
    let before = l.state().clone();
    let seq = l.seq();
    ops.arm(1, libc::EIO, "fsync segment");
    let r = l.append(Event::Campaign {
        ev: rustred_campaign::state::CampaignEvent::Drain,
        reason: "x".into(),
    });
    assert!(matches!(r, Err(LedgerError::Io { .. })), "{r:?}");
    assert!(l.is_poisoned());
    assert_eq!(l.state(), &before, "state not ahead of the disk (m3)");
    assert_eq!(l.seq(), seq);
    ops.disarm();
    // no later success on the poisoned handle, also not through sync()
    assert!(matches!(
        l.append(Event::Note { text: "x".into() }),
        Err(LedgerError::Poisoned { .. })
    ));
    assert!(matches!(l.sync(), Err(LedgerError::Poisoned { .. })));
    drop(l);
    // reopen: the batch was truncated away; the fsync gate re-verifies from disk
    let (l, _) = open_ledger(&d, as_dyn(&clk), cfg()).unwrap();
    assert_eq!(l.seq(), seq);
}

#[test]
fn a_m2_rotation_failure_never_fails_a_durable_append() {
    let d = scratch("m2");
    let _c = Cleanup(d.clone());
    let clk = clock();
    let small = LedgerConfig {
        rotate_bytes: 4096,
        rotation_retry_ms: 0,
        ..LedgerConfig::default()
    };
    let mut l = new_ledger(&d, as_dyn(&clk), small.clone());
    l.append_all(setup_events(N)).unwrap();
    let sealed0 = l.manifest().sealed.len();
    let ldir = ledger::ledger_dir(&d);
    fs::set_permissions(&ldir, fs::Permissions::from_mode(0o555)).unwrap();
    for k in 0..10 {
        l.append_all(item_life(N, k, 0, EvidenceLevel::GeometryAccepted, 10))
            .expect("durable appends are acknowledged");
    }
    fs::set_permissions(&ldir, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(
        l.warnings().iter().any(|w| w.contains("rotation failed")),
        "{:?}",
        l.warnings()
    );
    assert_eq!(l.manifest().sealed.len(), sealed0);
    let seq = l.seq();
    // retried later: rotation happens once the directory is writable again
    l.append_all(item_life(N, 10, 0, EvidenceLevel::GeometryAccepted, 10))
        .unwrap();
    assert!(l.manifest().sealed.len() > sealed0);
    drop(l);
    let (l2, _) = open_ledger(&d, as_dyn(&clk), small).unwrap();
    assert!(l2.seq() > seq);
}

#[test]
fn a_m3_evidence_levels_need_their_artefacts() {
    let (mut l, _c, _d) = with_items("m3", 0, EvidenceLevel::GeometryAccepted);
    l.append_all(item_until_produced(N, 0, 0, 10)).unwrap();
    let j = job_id(N, 0, 0);
    let mk = |level, evidence| {
        item(
            0,
            ItemEvent::Accept {
                rule: rule_id(N, 0),
                level,
                record: record_ref(N, 0),
                source: AcceptSource::Job {
                    job: j.clone(),
                    attempt: 1,
                    table_blake3: table_ref(&j, 1).blake3,
                },
                elided: false,
                evidence,
                round: None,
            },
        )
    };
    assert!(refused(l.append(mk(EvidenceLevel::IdentityProved, None))));
    assert!(refused(l.append(mk(
        EvidenceLevel::IdentityProved,
        level_evidence(EvidenceLevel::IdentityTested, "x")
    ))));
    l.append(mk(
        EvidenceLevel::IdentityProved,
        level_evidence(EvidenceLevel::IdentityProved, "x"),
    ))
    .unwrap();
    // upgrade must cite matching evidence too
    l.append_all(item_life(N, 1, 0, EvidenceLevel::GeometryAccepted, 10))
        .unwrap();
    assert!(refused(l.append(item(
        1,
        ItemEvent::Upgrade {
            level: EvidenceLevel::IdentityProved,
            evidence: LevelEvidence::IdentityTested { verdict: fref("v") },
        }
    ))));
}

#[test]
fn a_m4_terminals_need_an_exhausted_ladder_a_count_and_the_bound() {
    let (mut l, _c, _d) = with_items("m4", 0, EvidenceLevel::GeometryAccepted);
    l.append(item_created(N, 0, 0, None, "face")).unwrap();
    let propose = |preferred, exhausted| {
        item(
            0,
            ItemEvent::ProposeTerminal {
                reason: "x".into(),
                preferred,
                exhausted,
            },
        )
    };
    assert!(
        refused(l.append(propose(false, Some(fref("e"))))),
        "L0 is below the spec's terminal rung L1"
    );
    l.append(item_created(N, 1, 1, None, "face")).unwrap();
    assert!(
        refused(l.append(item(
            1,
            ItemEvent::ProposeTerminal {
                reason: "x".into(),
                preferred: false,
                exhausted: None
            }
        ))),
        "no ladder citation"
    );
    l.append(item(
        1,
        ItemEvent::ProposeTerminal {
            reason: "x".into(),
            preferred: false,
            exhausted: Some(fref("large:ladder/1.json")),
        },
    ))
    .unwrap();
    let declare = |count, keys: &str| {
        item(
            1,
            ItemEvent::DeclareTerminal {
                count,
                keys: fref(keys),
                reason: "x".into(),
                preferred: false,
            },
        )
    };
    assert!(refused(l.append(declare(0, "k"))), "count 0");
    assert!(refused(l.append(declare(101, "k"))), "beyond the bound");
    l.append(declare(2, "k")).unwrap();
    assert!(refused(l.append(declare(3, "k"))), "repeat must match");
    assert_eq!(l.state().nodes[N].state.terminals, 2);
    // a preferred terminal (R41 P-4) is allowed from the start and does not count against the bound
    l.append(propose(true, None)).unwrap();
    l.append(item(
        0,
        ItemEvent::DeclareTerminal {
            count: 5,
            keys: fref("pk"),
            reason: "preferred".into(),
            preferred: true,
        },
    ))
    .unwrap();
    let st = l.state();
    assert_eq!(st.nodes[N].state.terminals, 2);
    let r = st.terminal_report(N);
    assert_eq!((r.preferred_used, r.other), (5, 2));
}

#[test]
fn a_m5_boxed_only_after_a_real_full_attempt_and_never_after_full_closure() {
    let (mut l, _c, _d) = with_items("m5", 1, EvidenceLevel::IdentityProved);
    let ended = |cause| {
        node(NodeEvent::AttemptEnded {
            blockers: blockers(),
            reason: "x".into(),
            cause,
        })
    };
    assert!(
        refused(l.append(ended(AttemptEndCause::FixerRounds))),
        "never ran"
    );
    let st = l.state().clone();
    l.append(assembled(N, 1)).unwrap();
    l.append(closure_event(&st, N, 1, 9, EnvelopeKind::Proved))
        .unwrap();
    assert!(
        refused(l.append(ended(AttemptEndCause::FixerRounds))),
        "0 of 2 rounds"
    );
    assert!(
        refused(l.append(ended(AttemptEndCause::CpuBudget))),
        "CPU budget left"
    );
    assert!(refused(l.append(node(NodeEvent::AttemptEnded {
        blockers: Blockers::default(),
        reason: "x".into(),
        cause: AttemptEndCause::FixerRounds
    }))));
    l.append_all(vec![fixer_round(N, 1), fixer_round(N, 2)])
        .unwrap();
    l.append(ended(AttemptEndCause::FixerRounds)).unwrap();
    l.append(node(NodeEvent::FallBackToBox {
        box_id: format!("{N}.box"),
    }))
    .unwrap();
    // box pruning, then reopen to FULL: the box-only retirement is reinstated
    l.append(item(
        0,
        ItemEvent::Retire {
            rule: rule_id(N, 0),
            proof: RetirementProof::BoxClosurePass { closure: fref("c") },
            superseded_by: None,
        },
    ))
    .unwrap();
    l.append(node(NodeEvent::Reopen { reason: "r".into() }))
        .unwrap();
    assert_eq!(l.state().nodes[N].state.target, ClosureTarget::Full);
    assert!(matches!(
        l.state().items[&item_id(N, 0)].phase,
        ItemPhase::Accepted(_)
    ));
    // a node closed FULL is never boxed
    let (mut l, _c2, _d2) = with_items("m5b", 1, EvidenceLevel::IdentityProved);
    close(&mut l, 1);
    l.append(node(NodeEvent::Reopen { reason: "r".into() }))
        .unwrap();
    let st = l.state().clone();
    l.append(closure_event(&st, N, 2, 3, EnvelopeKind::Proved))
        .unwrap();
    l.append_all(vec![fixer_round(N, 1), fixer_round(N, 2)])
        .unwrap();
    l.append(ended(AttemptEndCause::FixerRounds)).unwrap();
    assert!(refused(l.append(node(NodeEvent::FallBackToBox {
        box_id: format!("{N}.box")
    }))));
}

#[test]
fn a_m6_write_handles_need_the_lock_and_read_only_never_repairs() {
    let (l, _c, d) = with_items("m6", 1, EvidenceLevel::GeometryAccepted);
    let clk = clock();
    // a second write handle while one is open: refused
    assert!(matches!(
        open_ledger(&d, as_dyn(&clk), cfg()),
        Err(LedgerError::Busy { .. })
    ));
    drop(l);
    // the lock of another directory: refused
    let other = scratch("m6-other");
    let _c2 = Cleanup(other.clone());
    assert!(matches!(
        Ledger::open(&d, lock_of(&other), as_dyn(&clk), cfg()),
        Err(LedgerError::WrongLock { .. })
    ));
    // read-only open of a torn tail reports it and changes nothing
    let seg = seg_path(&d);
    let mut b = fs::read(&seg).unwrap();
    b.extend_from_slice(b"{\"schema\":\"rustred.camp");
    fs::write(&seg, &b).unwrap();
    let v = ledger::open_read_only(&d).unwrap();
    assert!(v.unverified_tail.is_some());
    assert_eq!(fs::read(&seg).unwrap(), b);
}

#[test]
fn a_m7_verify_fails_without_progress_files_or_with_old_ones() {
    let (mut l, _c, d) = with_items("m7", 1, EvidenceLevel::GeometryAccepted);
    let r = progress::verify(&d).unwrap();
    assert!(!r.clean());
    assert_eq!(r.missing, vec!["campaign.json".to_string()]);
    let clk = clock();
    let mut w = ProgressWriter::new();
    w.write(&d, l.state(), l.head(), &Resources::default(), clk.as_ref())
        .unwrap();
    assert!(progress::verify(&d).unwrap().clean());
    l.append(Event::Note { text: "x".into() }).unwrap();
    let r = progress::verify(&d).unwrap();
    assert_eq!(r.stale.len(), 1, "older than the ledger tail");
}

// ------------------------------------------------------------------------------------------------------------
// Audit A: MINOR
// ------------------------------------------------------------------------------------------------------------

#[test]
fn a_m1_minor_old_attempt_records_are_refused() {
    let (mut l, _c, d) = with_items("mi1", 0, EvidenceLevel::GeometryAccepted);
    let life = item_life(N, 0, 0, EvidenceLevel::GeometryAccepted, 10);
    l.append_all(life[..4].to_vec()).unwrap();
    let j = job_id(N, 0, 0);
    l.append(Event::Job {
        job: j.clone(),
        attempt: 1,
        ev: JobEvent::Interrupt { reason: "x".into() },
    })
    .unwrap();
    l.append(Event::Job {
        job: j.clone(),
        attempt: 2,
        ev: JobEvent::Requeue { reason: "x".into() },
    })
    .unwrap();
    let len = fs::metadata(seg_path(&d)).unwrap().len();
    assert!(refused(l.append(Event::Job {
        job: j.clone(),
        attempt: 1,
        ev: finished_with(
            &j,
            1,
            FinishClass::RuleFound,
            ContractAccept::Canonical,
            Some(fref("late")),
            99
        ),
    })));
    assert_eq!(fs::metadata(seg_path(&d)).unwrap().len(), len);
}

#[test]
fn a_m2_minor_the_accepting_job_must_be_the_producing_job() {
    let (mut l, _c, _d) = with_items("mi2", 0, EvidenceLevel::GeometryAccepted);
    let life = item_until_produced(N, 0, 0, 10);
    l.append_all(life[..6].to_vec()).unwrap();
    // another sealed success of the same item
    new_job(&mut l, 0, "j-other");
    l.append(life[6].clone()).unwrap();
    assert!(refused(l.append(accept_from(
        0,
        "j-other",
        EvidenceLevel::GeometryAccepted
    ))));
}

#[test]
fn a_m3_minor_state_is_never_ahead_of_disk_after_a_write_error() {
    let d = scratch("mi3");
    let _c = Cleanup(d.clone());
    let clk = clock();
    let ops = Ops::new();
    let mut l =
        new_ledger_with(&d, as_dyn(&clk), cfg(), Arc::new(RecordingOps(ops.clone()))).unwrap();
    l.append_all(setup_events(N)).unwrap();
    let before = l.state().clone();
    ops.arm(1, libc::ENOSPC, "append segment");
    let r = l.append(item_created(N, 0, 0, None, "face"));
    assert!(matches!(r, Err(LedgerError::NoSpace { .. })), "{r:?}");
    assert_eq!(l.state(), &before);
}

#[test]
fn a_m4_minor_crashed_create_leaves_no_ledger_and_create_works_again() {
    let d = scratch("mi4");
    let _c = Cleanup(d.clone());
    let ld = ledger::ledger_dir(&d);
    fs::create_dir_all(&ld).unwrap();
    // a create that died before installing the manifest
    fs::write(ld.join("segment-000001.jsonl"), b"{partial").unwrap();
    fs::write(ld.join("COMMIT"), b"x").unwrap();
    let clk = clock();
    assert!(matches!(
        open_ledger(&d, as_dyn(&clk), cfg()),
        Err(LedgerError::NotALedger { .. })
    ));
    let l = new_ledger(&d, as_dyn(&clk), cfg());
    assert_eq!(l.seq(), 1);
}

#[test]
fn a_m5_minor_a_truncation_without_its_record_is_finished_on_open() {
    let (l, _c, d) = with_items("mi5", 1, EvidenceLevel::GeometryAccepted);
    let seq = l.seq();
    drop(l);
    // a crash after the truncation, before the recovery record: only the marker tells
    let ld = ledger::ledger_dir(&d);
    let len = fs::metadata(seg_path(&d)).unwrap().len();
    fs::write(ld.join("torn-x.bin"), b"{\"sch").unwrap();
    let marker = serde_json::json!({"info": {"segment": 1, "last_good_seq": seq, "truncated_bytes": 5,
        "saved_as": "torn-x.bin", "detail": "incomplete last line", "forced": false}, "truncate_to": len});
    fs::write(
        ld.join(ledger::RECOVERY_PENDING_FILE),
        serde_json::to_vec(&marker).unwrap(),
    )
    .unwrap();
    let clk = clock();
    let (l, rep) = open_ledger(&d, as_dyn(&clk), cfg()).unwrap();
    assert!(rep.recovery.is_some());
    assert_eq!(l.seq(), seq + 1);
    assert_eq!(l.state().recoveries, vec!["torn-x.bin".to_string()]);
    assert!(!ld.join(ledger::RECOVERY_PENDING_FILE).exists());
}

#[test]
fn a_m6_minor_renamed_host_and_own_record() {
    let d = scratch("mi6");
    let _c = Cleanup(d.clone());
    let me = lock::current_identity().unwrap();
    let write_owner = |id: lock::ProcessIdentity| {
        let o = lock::OwnerRecord {
            schema: lock::LOCK_SCHEMA.into(),
            identity: id,
            argv: vec!["old".into()],
            started_utc: T0.into(),
            token: "t".into(),
        };
        fs::write(d.join(lock::OWNER_FILE), serde_json::to_vec(&o).unwrap()).unwrap();
    };
    write_owner(lock::ProcessIdentity {
        host: "renamed.invalid".into(),
        ..me.clone()
    });
    let (lk, out) = lock::acquire(&d, &SystemClock::new(), &["x".into()]).unwrap();
    assert!(matches!(
        out,
        LockOutcome::TookOver {
            reason: DeadReason::OwnRecord,
            ..
        }
    ));
    drop(lk);
}

#[test]
fn a_m7_minor_finish_refused_while_jobs_run_or_intents_pend() {
    let (mut l, _c, _d) = with_items("mi7", 0, EvidenceLevel::GeometryAccepted);
    l.append_all(item_until_produced(N, 0, 0, 10)[..5].to_vec())
        .unwrap();
    let finish = Event::Campaign {
        ev: rustred_campaign::state::CampaignEvent::Finish,
        reason: "done".into(),
    };
    assert!(refused(l.append(finish.clone())), "a job is running");
    let j = job_id(N, 0, 0);
    l.append(Event::Job {
        job: j,
        attempt: 1,
        ev: JobEvent::Interrupt { reason: "x".into() },
    })
    .unwrap();
    l.append(Event::Intent {
        op: "o".into(),
        action: "write_keyset".into(),
        target: "k".into(),
    })
    .unwrap();
    assert!(refused(l.append(finish.clone())), "an intent pends");
    l.append(Event::Done {
        op: "o".into(),
        result: None,
    })
    .unwrap();
    l.append(finish).unwrap();
}

#[test]
fn a_m8_minor_non_canonical_lines_are_refused() {
    let (l, _c, d) = with_items("mi8", 0, EvidenceLevel::GeometryAccepted);
    drop(l);
    let seg = seg_path(&d);
    let b = fs::read(&seg).unwrap();
    let (s, e) = last_line_range(&b);
    let (rec, _) = ledger::decode_record(&b[s..e]).unwrap();
    // an extra field, re-digested so only the canonical-form check can catch it
    let mut v = serde_json::to_value(&rec).unwrap();
    v["extra"] = serde_json::json!(1);
    let body = serde_json::to_vec(&v).unwrap();
    let h = ledger::line_hash(&body);
    let mut line = body[..body.len() - 1].to_vec();
    line.extend_from_slice(format!(",\"h\":\"{h}\"}}").as_bytes());
    assert!(
        ledger::decode_record(&line)
            .unwrap_err()
            .contains("canonical")
    );
}

#[test]
fn a_m9_minor_unchanged_progress_files_are_skipped() {
    let (mut l, _c, d) = with_items("mi9", 1, EvidenceLevel::GeometryAccepted);
    let clk = clock();
    let mut w = ProgressWriter::new();
    w.write(&d, l.state(), l.head(), &Resources::default(), clk.as_ref())
        .unwrap();
    l.append(Event::Note { text: "x".into() }).unwrap();
    let r = w
        .write(&d, l.state(), l.head(), &Resources::default(), clk.as_ref())
        .unwrap();
    assert!(!r.written.iter().any(|f| f.starts_with("nodes/")), "{r:?}");
}

#[test]
fn a_m10_minor_unmeasured_resources_are_null() {
    let (l, _c, _d) = with_items("mi10", 0, EvidenceLevel::GeometryAccepted);
    let set = progress::derive(l.state(), l.head(), &Resources::default(), T0);
    let r = &set["campaign.json"]["resources"];
    assert!(r["cpus_in_use"].is_null() && r["host_mem_available_bytes"].is_null());
}

#[test]
fn a_m11_minor_node_stale_marks_items_and_escalated_items_are_not_revived() {
    let (mut l, _c, _d) = with_items("mi11", 1, EvidenceLevel::GeometryAccepted);
    l.append_all(item_until_produced(N, 1, 0, 10)).unwrap();
    l.append(item(
        1,
        ItemEvent::Reject {
            reason: "r".into(),
            disproof: None,
        },
    ))
    .unwrap();
    l.append(item(
        1,
        ItemEvent::Escalate {
            to: Rung(1),
            reason: "r".into(),
        },
    ))
    .unwrap();
    l.append(item_created(N, 2, 1, Some(item_id(N, 1)), "face"))
        .unwrap();
    l.append(node(NodeEvent::MarkStale {
        reason: "spec amended".into(),
    }))
    .unwrap();
    assert!(
        l.state()
            .items
            .values()
            .all(|i| i.phase == ItemPhase::Stale)
    );
    l.append(node(NodeEvent::Reopen { reason: "r".into() }))
        .unwrap();
    l.append(item(2, ItemEvent::Reopen { reason: "r".into() }))
        .unwrap();
    assert!(
        refused(l.append(item(1, ItemEvent::Reopen { reason: "r".into() }))),
        "its child carries the work"
    );
}

#[test]
fn a_m12_minor_failed_attempts_are_counted_apart_from_interrupts() {
    let (mut l, _c, _d) = with_items("mi12", 0, EvidenceLevel::GeometryAccepted);
    l.append_all(item_until_produced(N, 0, 0, 10)[..4].to_vec())
        .unwrap();
    let j = job_id(N, 0, 0);
    let ev = |attempt, ev| Event::Job {
        job: j.clone(),
        attempt,
        ev,
    };
    l.append_all(vec![
        ev(
            1,
            JobEvent::Interrupt {
                reason: "operator stop".into(),
            },
        ),
        ev(
            2,
            JobEvent::Requeue {
                reason: "resume".into(),
            },
        ),
        ev(
            2,
            JobEvent::Launch {
                run_dir: "r2".into(),
                cpus: vec![],
                reserved_bytes: 1,
            },
        ),
        ev(
            2,
            finished_with(
                &j,
                2,
                FinishClass::Crashed,
                ContractAccept::Nothing,
                None,
                5,
            ),
        ),
        ev(
            3,
            JobEvent::Retry {
                reason: "retry once".into(),
            },
        ),
    ])
    .unwrap();
    let st = l.state();
    assert_eq!(st.jobs[&j].attempt, 3);
    assert_eq!(st.jobs[&j].failures, 1, "the interrupt never counts (U-5)");
    assert_eq!(st.items[&item_id(N, 0)].failed_attempts, 1);
}

// ------------------------------------------------------------------------------------------------------------
// Audit B
// ------------------------------------------------------------------------------------------------------------

#[test]
fn b_exp1_internal_node_events_cannot_be_records() {
    // the reducer-internal transitions are no longer variants of the record enum: such a line does not decode
    let body = br#"{"node":"T162","ev":{"ev":"terminal_declared","count":3}}"#;
    assert!(
        serde_json::from_slice::<Event>(
            &[
                br#"{"kind":"node","body":"#.as_slice(),
                body.as_slice(),
                b"}"
            ]
            .concat()
        )
        .is_err()
    );
}

#[test]
fn b_exp1_no_new_attempt_from_queued_and_no_produce_of_a_foreign_table() {
    let (mut l, _c, _d) = with_items("b1", 0, EvidenceLevel::GeometryAccepted);
    let life = item_until_produced(N, 0, 0, 10);
    l.append_all(life[..3].to_vec()).unwrap();
    let j = job_id(N, 0, 0);
    assert!(refused(l.append(Event::Job {
        job: j.clone(),
        attempt: 2,
        ev: JobEvent::Requeue { reason: "x".into() }
    })));
    assert!(refused(l.append(Event::Job {
        job: j.clone(),
        attempt: 2,
        ev: JobEvent::Retry { reason: "x".into() }
    })));
    l.append_all(life[3..6].to_vec()).unwrap();
    assert!(refused(l.append(item(
        0,
        ItemEvent::Produce {
            job: j,
            attempt: 1,
            table: fref("another table"),
        }
    ))));
}

#[test]
fn b_exp1_re_registration_and_re_import_must_be_identical() {
    let (mut l, _c, _d) = with_items("b1r", 0, EvidenceLevel::GeometryAccepted);
    let mut reg = setup_events(N)[5].clone();
    if let Event::NodeRegistered { terminal_bound, .. } = &mut reg {
        *terminal_bound = Some(7);
    }
    assert!(refused(l.append(reg)));
    let ev = |description: &str| Event::EvidenceImported {
        evidence: "e".into(),
        producer: "p".into(),
        description: description.into(),
        files: vec![fref("f")],
        level_claimed: EvidenceLevel::GeometryAccepted,
    };
    l.append(ev("one")).unwrap();
    assert!(refused(l.append(ev("two"))));
}

#[test]
fn b_exp2_damage_in_any_sealed_segment_refuses_open_and_read_records_never_gaps() {
    let d = scratch("b2");
    let _c = Cleanup(d.clone());
    let clk = clock();
    let small = LedgerConfig {
        rotate_bytes: 4096,
        ..LedgerConfig::default()
    };
    let mut l = new_ledger(&d, as_dyn(&clk), small.clone());
    l.append_all(setup_events(N)).unwrap();
    for k in 0..20 {
        l.append_all(item_life(N, k, 0, EvidenceLevel::GeometryAccepted, 10))
            .unwrap();
    }
    assert!(l.manifest().sealed.len() >= 2);
    let first = ledger::ledger_dir(&d).join(&l.manifest().sealed[0].file);
    drop(l);
    // a flip inside the FIRST sealed segment (not the last): open refuses
    fs::set_permissions(&first, fs::Permissions::from_mode(0o644)).unwrap();
    let mut b = fs::read(&first).unwrap();
    let orig = b.clone();
    b[100] ^= 1;
    fs::write(&first, &b).unwrap();
    assert!(matches!(
        open_ledger(&d, as_dyn(&clk), small.clone()),
        Err(LedgerError::Corrupt { .. })
    ));
    fs::write(&first, &orig).unwrap();
    // a deleted middle line in the active segment: read_records is an error, not a gapped history
    let (_l, _c2, d) = with_items("b2b", 3, EvidenceLevel::GeometryAccepted);
    drop(_l);
    let seg = seg_path(&d);
    let a = fs::read(&seg).unwrap();
    let lines: Vec<&[u8]> = a.split(|&c| c == b'\n').filter(|l| !l.is_empty()).collect();
    let mut cut = Vec::new();
    for (i, x) in lines.iter().enumerate() {
        if i != 1 {
            cut.extend_from_slice(x);
            cut.push(b'\n');
        }
    }
    fs::write(&seg, &cut).unwrap();
    assert!(ledger::read_records(&d).is_err());
}

// ------------------------------------------------------------------------------------------------------------
// R40 / R41 rulings on the records
// ------------------------------------------------------------------------------------------------------------

#[test]
fn u9_and_genericity_no_master_counts_or_node_names_in_the_code() {
    for (name, src) in [
        ("state.rs", include_str!("../src/state.rs")),
        ("ledger.rs", include_str!("../src/ledger.rs")),
        ("progress.rs", include_str!("../src/progress.rs")),
    ] {
        let code: String = src
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        let code = code.split("#[cfg(test)]").next().unwrap();
        for w in [
            "master_count",
            "knock_out",
            "knockout",
            "lp.tsv",
            "T162",
            "T178",
            "T266",
        ] {
            assert!(!code.contains(w), "{name} contains {w:?}");
        }
    }
}

#[test]
fn u12_certification_is_a_per_node_spec_value() {
    // FULL default replay: identity_tested rules certify
    let (mut l, _c, _d) = with_items("u12", 1, EvidenceLevel::IdentityTested);
    close(&mut l, 1);
    l.append(node(NodeEvent::DeclareCertified)).unwrap();
    let set = progress::derive(l.state(), l.head(), &Resources::default(), T0);
    assert_eq!(set["nodes/T162.json"]["certification"]["asked"], "replay");
    assert_eq!(set["nodes/T162.json"]["certification"]["reached"], "replay");
    // a spec asking per-key on FULL: identity_tested does not certify
    let d = scratch("u12b");
    let _c2 = Cleanup(d.clone());
    let clk = clock();
    let mut l = new_ledger(&d, as_dyn(&clk), cfg());
    let mut setup = setup_events(N);
    if let Event::NodeRegistered { spec, .. } = &mut setup[5] {
        spec.certify_full = Some(rustred_campaign::state::CertLevel::PerKey);
    }
    l.append_all(setup).unwrap();
    l.append_all(item_life(N, 0, 0, EvidenceLevel::IdentityTested, 10))
        .unwrap();
    close(&mut l, 1);
    assert!(refused(l.append(node(NodeEvent::DeclareCertified))));
}

#[test]
fn u13_optimise_mode_replacements_need_economy_and_containment() {
    let d = scratch("u13");
    let _c = Cleanup(d.clone());
    let clk = clock();
    let mut g = Genesis::new("opt", &digest("spec"), 100);
    g.mode = CampaignMode::Optimise;
    let mut l = Ledger::create(&d, lock_of(&d), as_dyn(&clk), cfg(), g).unwrap();
    l.append_all(setup_events(N)).unwrap();
    l.append(Event::TablesImported {
        provenance: "src".into(),
        producer: "earlier campaign".into(),
        source_campaign: Some("c0".into()),
        manifest: fref("large:c0/MANIFEST.json"),
        files: vec![],
        level_claimed: EvidenceLevel::IdentityTested,
    })
    .unwrap();
    let econ = |rhs| RuleEconomy {
        cases: 1,
        rules: 1,
        rhs_terms: rhs,
        monomials: 10,
        coefficient_degree: 2,
        bytes: 100,
    };
    for k in 0..2 {
        l.append(item_created(N, k, 0, None, "face")).unwrap();
        let imp = |level| {
            item(
                k,
                ItemEvent::Import {
                    rule: rule_id(N, k),
                    level,
                    record: record_ref(N, k),
                    provenance: "src".into(),
                },
            )
        };
        assert!(
            refused(l.append(imp(EvidenceLevel::IdentityProved))),
            "capped by the import claim"
        );
        l.append(imp(EvidenceLevel::IdentityTested)).unwrap();
        l.append(Event::RuleEconomy {
            item: item_id(N, k),
            economy: econ(500),
        })
        .unwrap();
    }
    // an optimise item produces a smaller rule
    l.append_all(item_life(N, 2, 0, EvidenceLevel::IdentityTested, 10))
        .unwrap();
    l.append(Event::RuleEconomy {
        item: item_id(N, 2),
        economy: econ(1200),
    })
    .unwrap();
    let replace = item(
        2,
        ItemEvent::Replace {
            replaces: vec![item_id(N, 0), item_id(N, 1)],
            containment: fref("large:proofs/containment.json"),
        },
    );
    assert!(
        refused(l.append(replace.clone())),
        "1200 > 1000 rhs terms: not better"
    );
    assert!(
        refused(l.append(item(
            0,
            ItemEvent::Retire {
                rule: rule_id(N, 0),
                proof: RetirementProof::ReplacedBy { proof: fref("p") },
                superseded_by: Some(item_id(N, 2)),
            }
        ))),
        "optimise mode: only through replace"
    );
    l.append(Event::RuleEconomy {
        item: item_id(N, 2),
        economy: econ(700),
    })
    .unwrap();
    l.append(replace).unwrap();
    let e = l.state().node_economy(N);
    assert_eq!((e.rules_before, e.rules_after), (3, 1));
    assert_eq!((e.before.rhs_terms, e.after.rhs_terms), (1700, 700));
}

#[test]
fn r41_provisional_closure_against_measured_ranges() {
    let (mut l, _c, _d) = with_items("p2", 1, EvidenceLevel::IdentityTested);
    let st = l.state().clone();
    l.append(assembled(N, 1)).unwrap();
    l.append(closure_event(&st, N, 1, 0, EnvelopeKind::Measured))
        .unwrap();
    l.append(node(NodeEvent::DeclareClosed)).unwrap();
    assert!(l.state().nodes[N].state.provisional());
    let recheck = |passed| {
        node(NodeEvent::EnvelopeRecheck {
            passed,
            report: fref("large:recheck.json"),
            envelope_tables: vec![fref("large:proved-ranges.json")],
        })
    };
    l.append(recheck(true)).unwrap();
    assert!(!l.state().nodes[N].state.provisional());
    assert_eq!(l.state().nodes[N].state.phase, NodePhase::Closed);
    // a failed recheck reopens
    let (mut l, _c2, _d2) = with_items("p2b", 1, EvidenceLevel::IdentityTested);
    let st = l.state().clone();
    l.append(assembled(N, 1)).unwrap();
    l.append(closure_event(&st, N, 1, 0, EnvelopeKind::Measured))
        .unwrap();
    l.append(node(NodeEvent::DeclareClosed)).unwrap();
    l.append(recheck(false)).unwrap();
    assert_eq!(l.state().nodes[N].state.phase, NodePhase::Running);
}

#[test]
fn operator_controls_and_attention() {
    let (mut l, _c, _d) = with_items("ctl", 0, EvidenceLevel::GeometryAccepted);
    l.append(item_created(N, 0, 0, None, "face")).unwrap();
    l.append(item(
        0,
        ItemEvent::Attention {
            code: "needs_bigmem".into(),
            text: "t".into(),
        },
    ))
    .unwrap();
    assert!(
        refused(l.append(item(0, ItemEvent::Start { job: None }))),
        "needs a person"
    );
    let ctl = |id: &str, ev| Event::Control { id: id.into(), ev };
    let req = |verb, target| ControlEvent::Requested {
        verb,
        target,
        by: "op".into(),
        reason: "r".into(),
    };
    l.append(ctl(
        "c1",
        req(
            ControlVerb::Retry,
            ControlTarget::Item {
                item: item_id(N, 0),
            },
        ),
    ))
    .unwrap();
    let applied = || ControlEvent::Applied {
        effect: "done".into(),
    };
    l.append(ctl("c1", applied())).unwrap();
    let it = &l.state().items[&item_id(N, 0)];
    assert_eq!(it.phase, ItemPhase::Open);
    assert_eq!(
        (it.operator_retries, it.failed_attempts, it.generation),
        (1, 0, 1)
    );
    // a refused control cannot be applied afterwards
    l.append(ctl("c2", req(ControlVerb::Pause, ControlTarget::Campaign)))
        .unwrap();
    l.append(ctl("c2", ControlEvent::Refused { why: "no".into() }))
        .unwrap();
    assert!(refused(l.append(ctl("c2", applied()))));
    assert!(!l.state().paused);
    // pause an item: its jobs are not launched
    l.append(ctl(
        "c3",
        req(
            ControlVerb::Pause,
            ControlTarget::Item {
                item: item_id(N, 0),
            },
        ),
    ))
    .unwrap();
    l.append(ctl("c3", applied())).unwrap();
    l.append(job_queued("jp", &item_id(N, 0), 0, "generate"))
        .unwrap();
    assert!(refused(l.append(Event::Job {
        job: "jp".into(),
        attempt: 1,
        ev: JobEvent::Launch {
            run_dir: "r".into(),
            cpus: vec![],
            reserved_bytes: 1
        }
    })));
    // skip
    l.append(ctl(
        "c4",
        req(
            ControlVerb::Skip,
            ControlTarget::Item {
                item: item_id(N, 0),
            },
        ),
    ))
    .unwrap();
    l.append(ctl("c4", applied())).unwrap();
    assert_eq!(l.state().items[&item_id(N, 0)].phase, ItemPhase::Skipped);
    assert_eq!(
        l.state().nodes[N].context().items_open,
        1,
        "skipped work stays open"
    );
}
