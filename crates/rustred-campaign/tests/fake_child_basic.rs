//! Fake child, normal paths (child-job.md section 4: F1, F2, F13 and the brief's scenarios 1, 2, 3, 12, 13, 14):
//! every scenario runs `rr_fake_child` as a real process and classifies its files with the contract's table.

#[path = "fake_child_common/mod.rs"]
mod common;
use common::*;
use rustred_campaign_contract::classify::{Accept, KillReason, OutcomeKind, WaitStatus};
use rustred_campaign_contract::events::Event;
use rustred_campaign_contract::fake::*;
use rustred_campaign_contract::outcome::TimeDecisionWhat;
use rustred_campaign_contract::seal::{verify_sealed, SealVerdict};
use rustred_campaign_contract::spec::*;
use rustred_campaign_contract::{ExitCode, StopCode};
use std::time::Duration;

const T: Duration = Duration::from_secs(30);

#[test]
fn f1_success_after_heartbeats_with_rules() {
    // brief 1 / F1: succeed after N heartbeats with given rule records -> row 1
    let mut root = Root::new("f1");
    let dir = root.run_dir("f1", 1);
    let s = spec(&dir, "f1", 1, script(vec![FakeOp::Prepare { ms: 20 }, rule_case(0, 50), covered_case(1, 10), rule_case(2, 50), FakeOp::Heartbeats { n: 2 }, end(None)]));
    let (r, w) = run_to_end(&mut root, &s, T);
    assert_eq!(w, WaitStatus::Exited(0), "{}", r.stderr());
    let c = r.classify(w, KillReason::None);
    assert_eq!((c.row, c.kind, c.accept), (1, OutcomeKind::Complete, Accept::Canonical));
    let ev = r.events();
    assert!(ev.iter().filter(|e| matches!(e.event, Event::Heartbeat { .. })).count() >= 2);
    assert!(matches!(ev[0].event, Event::JobStarted { .. }));
    assert!(matches!(ev.last().unwrap().event, Event::JobFinished { exit_code: ExitCode::Complete, .. }));
    assert!(ev.windows(2).all(|p| p[1].seq == p[0].seq + 1), "seq order");
    let o = r.outcome();
    assert_eq!(o.job.counters.rules_committed, 2);
    assert_eq!(o.job.counters.cases, 3);
    let t = o.job.outputs.table.clone().unwrap();
    assert_eq!(verify_sealed(&r.layout.table(), &t), SealVerdict::Sealed);
    assert_eq!((t.records, t.rules), (Some(4), Some(2)));
    assert!(!r.layout.reserve().exists(), "reserve released at the end");
    // determinism: a second run of the same spec in a new RUN gives the same canonical table and digest
    let dir2 = root.run_dir("f1", 2);
    let mut s2 = s.clone();
    s2.attempt = 2;
    s2.run_dir = dir2.to_str().unwrap().into();
    let (r2, w2) = run_to_end(&mut root, &s2, T);
    assert_eq!(w2, WaitStatus::Exited(0));
    assert_eq!(canonical_of(&r), canonical_of(&r2));
    assert_eq!(std::fs::read(r.layout.table()).unwrap(), std::fs::read(r2.layout.table()).unwrap(), "no jitter: byte-identical");
    root.assert_no_process_left();
}

#[test]
fn f2_end_with_each_stop_code() {
    // F2 / brief 2: `end` with each stop code -> rows 2, 3/24, 4, 5, 6, 8, 11 one by one
    let mut root = Root::new("f2end");
    let cases: Vec<(Vec<FakeOp>, Option<StopCode>, u8)> = vec![
        (vec![rule_case(0, 0)], Some(StopCode::MaxCases), 2),
        (vec![covered_case(0, 0)], Some(StopCode::MaxCases), 3),
        (vec![FakeOp::Case { ordinal: 0, ms: 0, work: Default::default(), result: rustred_campaign_contract::stop::CaseResult::Refused, rule: None, children: vec![] }], Some(StopCode::MaxCases), 24),
        (vec![], Some(StopCode::LiftRefused), 4),
        (vec![], Some(StopCode::GuardTimeout), 4),
        (vec![], Some(StopCode::NumericSearchRefused), 4),
        (vec![], Some(StopCode::BeforeNumeric), 4),
        (vec![rule_case(0, 0)], Some(StopCode::OpenRegions), 5),
        (vec![rule_case(0, 0)], Some(StopCode::OpenObligations), 5),
        (vec![], Some(StopCode::NotLower), 6),
        (vec![rule_case(0, 0)], Some(StopCode::BudgetWork), 8),
        (vec![], Some(StopCode::Interrupted), 11),
        (vec![], Some(StopCode::BudgetMemory), 10),
    ];
    for (k, (mut ops, stop, row)) in cases.into_iter().enumerate() {
        let job = format!("e{k}");
        let dir = root.run_dir(&job, 1);
        ops.push(end(stop));
        let s = spec(&dir, &job, 1, script(ops));
        let (r, w) = run_to_end(&mut root, &s, T);
        assert_eq!(w, WaitStatus::Exited(stop.unwrap().exit_code().code()), "{stop:?}: {}", r.stderr());
        let c = r.classify(w, KillReason::None);
        assert_eq!(c.row, row, "{stop:?}: {c:?}");
    }
    // checker-like verdicts 11 / 12 -> row 7
    for (k, v) in [ExitCode::DecidedNegative, ExitCode::Undecided].into_iter().enumerate() {
        let job = format!("v{k}");
        let dir = root.run_dir(&job, 1);
        let s = spec(&dir, &job, 1, script(vec![FakeOp::End { stop: None, verdict: Some(v) }]));
        let (r, w) = run_to_end(&mut root, &s, T);
        assert_eq!(w, WaitStatus::Exited(v.code()));
        assert_eq!(r.classify(w, KillReason::None).row, 7);
    }
    root.assert_no_process_left();
}

#[test]
fn f2_fail_with_each_error_code() {
    // brief 2: fail with a given code (OUTCOME with `error`) -> rows 12, 13, 14, 16, 17
    let mut root = Root::new("f2fail");
    for (code, row) in [(ExitCode::Usage, 12), (ExitCode::Input, 13), (ExitCode::Refused, 14), (ExitCode::Io, 16), (ExitCode::Internal, 17)] {
        let job = format!("x{}", code.code());
        let dir = root.run_dir(&job, 1);
        let s = spec(&dir, &job, 1, script(vec![rule_case(0, 0), FakeOp::Fail { code, message: "scripted".into() }]));
        let (r, w) = run_to_end(&mut root, &s, T);
        assert_eq!(w, WaitStatus::Exited(code.code()));
        let o = r.outcome();
        assert_eq!(o.job.exit_code, code);
        assert!(o.job.error.is_some());
        assert_eq!(r.classify(w, KillReason::None).row, row, "{code}");
    }
    root.assert_no_process_left();
}

#[test]
fn f2_exit_without_outcome_is_crashed() {
    // F2: `exit` with codes outside / inside the table but without OUTCOME -> row 23 (or the code's own row)
    let mut root = Root::new("f2exit");
    for (code, row) in [(0, 23), (1, 23), (101, 23), (10, 23), (42, 14), (50, 15)] {
        let job = format!("c{code}");
        let dir = root.run_dir(&job, 1);
        let mut sc = script(vec![rule_case(0, 0), end(None)]);
        sc.faults = vec![Fault { at_op: 1, kind: FaultKind::Exit, bytes: 0, code, path: None }];
        let s = spec(&dir, &job, 1, sc);
        let (r, w) = run_to_end(&mut root, &s, T);
        assert_eq!(w, WaitStatus::Exited(code));
        assert!(!r.layout.outcome().exists());
        let c = r.classify(w, KillReason::None);
        assert_eq!(c.row, row, "exit {code}");
        if row == 23 {
            assert!(c.resume_from_r, "RESUME of the last commit is valid");
        }
    }
    root.assert_no_process_left();
}

#[test]
fn budget_stop_by_work_counter_is_exact_and_deterministic() {
    // brief 3: budget stop by a work counter: fires exactly at its limit, exit 20, sealed partial -> row 8
    let mut root = Root::new("budget");
    let mut results = vec![];
    for a in 1..=2 {
        let dir = root.run_dir("b", a);
        let mut s = spec(&dir, "b", a, script(vec![rule_case(0, 0), FakeOp::Case { ordinal: 1, ms: 300, work: [(WorkCounter::Seeds, 1000), (WorkCounter::Rows, 50)].into_iter().collect(), result: rustred_campaign_contract::stop::CaseResult::Covered, rule: None, children: vec![] }, end(None)]));
        s.budgets.work.seeds_per_case = Some(400);
        let (r, w) = run_to_end(&mut root, &s, T);
        assert_eq!(w, WaitStatus::Exited(20), "{}", r.stderr());
        let c = r.classify(w, KillReason::None);
        assert_eq!((c.row, c.kind, c.accept), (8, OutcomeKind::Spill, Accept::CommittedRules));
        let o = r.outcome();
        let hit = o.job.budgets.hit.clone().unwrap();
        assert_eq!((hit.counter, hit.scope, hit.limit, hit.value), (WorkCounter::Seeds, Scope::Case, 400, 400));
        assert_eq!(o.job.stop.code, Some(StopCode::BudgetWork));
        // counters: case 0 did 100 seeds, case 1 stopped at exactly 400
        assert_eq!(o.job.counters.seeds, 500);
        assert!(r.events().iter().any(|e| matches!(e.event, Event::BudgetHit { value: 400, .. })));
        results.push((o.job.counters, canonical_of(&r)));
    }
    assert_eq!(results[0], results[1], "same counters and canonical table twice");
    // on_case_budget: open -> the case becomes an obligation of kind budget and the queue continues
    let dir = root.run_dir("bo", 1);
    let mut s = spec(&dir, "bo", 1, script(vec![FakeOp::Case { ordinal: 0, ms: 0, work: [(WorkCounter::Seeds, 1000)].into_iter().collect(), result: rustred_campaign_contract::stop::CaseResult::Covered, rule: None, children: vec![] }, rule_case(1, 0), end(None)]));
    s.budgets.work.seeds_per_case = Some(500); // case 0 needs 1000 (budget), case 1 needs 200
    s.on_case_budget = OnCaseBudget::Open;
    let (r, w) = run_to_end(&mut root, &s, T);
    assert_eq!(w, WaitStatus::Exited(0));
    let o = r.outcome();
    assert_eq!(o.job.per_case[0].result, rustred_campaign_contract::stop::CaseResult::Budget);
    assert_eq!(o.job.counters.rules_committed, 1);
    root.assert_no_process_left();
}

fn loci_script() -> FakeScript {
    let mut sc = script(vec![FakeOp::Case {
        ordinal: 0,
        ms: 0,
        work: Default::default(),
        result: rustred_campaign_contract::stop::CaseResult::Rule,
        rule: Some(FakeRule { record: serde_json::json!({"target":"I(1)","rhs":[]}), loci: Some(FakeLoci { n: 3, len: 6000 }) }),
        children: vec![],
    }]);
    sc.script.push(end(None));
    sc
}

#[test]
fn witness_loci_full_when_forbidden_elided_when_allowed() {
    // brief 12 / ruling R-f: elision forbidden (default) -> full loci; allowed -> "elided" and the campaign (which
    // forbids) treats such a record as unusable (row 22)
    let mut root = Root::new("loci");
    let dir = root.run_dir("lf", 1);
    let s = spec(&dir, "lf", 1, loci_script());
    assert!(s.elision_forbidden());
    let (r, w) = run_to_end(&mut root, &s, T);
    assert_eq!(w, WaitStatus::Exited(0));
    let table = std::fs::read_to_string(r.layout.table()).unwrap();
    let rule: serde_json::Value = serde_json::from_str(table.lines().nth(1).unwrap()).unwrap();
    let loci = rule["witness_loci"].as_array().unwrap();
    assert_eq!(loci.len(), 3);
    assert!(loci.iter().all(|l| l.as_str().unwrap().len() == 6000), "written in full beyond the 4000-character limit");
    assert!(!record_has_elided_loci(&rule));
    assert_eq!(r.outcome().job.outputs.table.unwrap().witness_loci_elided, 0);
    assert_eq!(r.classify(w, KillReason::None).row, 1);

    let dir = root.run_dir("la", 1);
    let mut s = spec(&dir, "la", 1, loci_script());
    s.output.elide_witness_loci = true;
    let (r, w) = run_to_end(&mut root, &s, T);
    assert_eq!(w, WaitStatus::Exited(0));
    let table = std::fs::read_to_string(r.layout.table()).unwrap();
    let rule: serde_json::Value = serde_json::from_str(table.lines().nth(1).unwrap()).unwrap();
    assert!(record_has_elided_loci(&rule));
    assert_eq!(r.outcome().job.outputs.table.unwrap().witness_loci_elided, 3);
    // the spec allowed it: classification under the spec's own option is row 1 ...
    assert_eq!(r.classify(w, KillReason::None).row, 1);
    // ... but under the campaign's rule (forbid) the record is unusable: fail closed
    let mut e = rustred_campaign_contract::classify::assess(&r.layout, &r.spec, w, KillReason::None, false);
    e.elision_forbidden = true;
    assert_eq!(rustred_campaign_contract::classify::classify(&e).row, 22);
    root.assert_no_process_left();
}

#[test]
fn slow_start() {
    // brief 13: slow start: nothing written during the delay (the supervisor's "no job_started after 60 s" rule
    // would apply), then a normal complete run
    let mut root = Root::new("slow");
    let dir = root.run_dir("s", 1);
    let mut sc = script(vec![rule_case(0, 0), end(None)]);
    sc.start_delay_ms = 1200;
    let s = spec(&dir, "s", 1, sc);
    let mut r = spawn(&mut root, &s);
    std::thread::sleep(Duration::from_millis(600));
    assert!(!r.layout.events().exists(), "no EVENTS during the slow start");
    assert!(r.child.try_wait().unwrap().is_none());
    let w = r.wait(T);
    assert_eq!(w, WaitStatus::Exited(0));
    let ev = r.events();
    assert!(ev[0].t < 1.0 && matches!(ev[0].event, Event::JobStarted { .. }));
    assert!(r.started.elapsed() >= Duration::from_millis(1200));
    assert_eq!(r.classify(w, KillReason::None).row, 1);
    root.assert_no_process_left();
}

#[test]
fn volatile_jitter_changes_bytes_not_canonical_digest() {
    // brief 14: nondeterministic volatile fields only: two runs differ byte-wise, same canonical digest
    let mut root = Root::new("jitter");
    let mut runs = vec![];
    for a in 1..=2 {
        let dir = root.run_dir("j", a);
        let mut sc = script(vec![rule_case(0, 0), rule_case(1, 0), end(Some(StopCode::MaxCases))]);
        sc.volatile_jitter = true;
        let s = spec(&dir, "j", a, sc);
        let (r, w) = run_to_end(&mut root, &s, T);
        assert_eq!(w, WaitStatus::Exited(10));
        runs.push(r);
    }
    let (a, b) = (std::fs::read(runs[0].layout.table()).unwrap(), std::fs::read(runs[1].layout.table()).unwrap());
    assert_ne!(a, b, "volatile values differ");
    let (ta, tb) = (runs[0].outcome().job.outputs.table.unwrap(), runs[1].outcome().job.outputs.table.unwrap());
    assert_ne!(ta.blake3, tb.blake3);
    assert_eq!(ta.canonical_blake3, tb.canonical_blake3);
    assert_eq!(canonical_of(&runs[0]), canonical_of(&runs[1]));
    root.assert_no_process_left();
}

#[test]
fn time_decision_marks_the_job_time_decided() {
    // F13: a fake time decision -> OUTCOME time_decided, classification carries the flag
    let mut root = Root::new("td");
    let dir = root.run_dir("t", 1);
    let s = spec(&dir, "t", 1, script(vec![FakeOp::TimeDecision { what: TimeDecisionWhat::GuardRoute, limit_s: 60.0, measured_s: 61.0, taken: "content".into() }, rule_case(0, 0), end(None)]));
    let (r, w) = run_to_end(&mut root, &s, T);
    assert_eq!(w, WaitStatus::Exited(0));
    let o = r.outcome();
    assert!(o.job.time_decided);
    let c = r.classify(w, KillReason::None);
    assert_eq!((c.row, c.time_decided), (1, true));
    assert!(r.events().iter().any(|e| matches!(e.event, Event::TimeDecision(_))));
    root.assert_no_process_left();
}

#[test]
fn refusals_and_version() {
    // contract 3.1.3: OUTCOME exists -> 42 and the existing OUTCOME is not touched; version --json
    let mut root = Root::new("refuse");
    let dir = root.run_dir("r", 1);
    std::fs::write(dir.join("OUTCOME.json"), b"{\"keep\":1}").unwrap();
    let s = spec(&dir, "r", 1, script(vec![end(None)]));
    let (r, w) = run_to_end(&mut root, &s, T);
    assert_eq!(w, WaitStatus::Exited(42));
    assert_eq!(std::fs::read(dir.join("OUTCOME.json")).unwrap(), b"{\"keep\":1}");
    assert_eq!(r.classify(w, KillReason::None).row, 14);
    // invalid spec content: 40 with an OUTCOME carrying the error
    let dir = root.run_dir("u", 1);
    let mut s = spec(&dir, "u", 1, script(vec![end(None)]));
    s.args = vec!["--x".into()];
    let (r, w) = run_to_end(&mut root, &s, T);
    assert_eq!(w, WaitStatus::Exited(40));
    assert_eq!(r.outcome().job.exit_code, ExitCode::Usage);
    let out = std::process::Command::new(BIN).args(["version", "--json"]).output().unwrap();
    let v: rustred_campaign_contract::VersionInfo = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v.contract, vec![rustred_campaign_contract::CONTRACT.to_string()]);
    assert_eq!(v.job_kinds, vec![JobKind::Fake]);
    assert!(!v.git_commit.is_empty());
    root.assert_no_process_left();
}
