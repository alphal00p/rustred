//! Fake child, faults (child-job.md section 4: F5, F6, F7, F8, F11, F12; the brief's scenarios 6-10, 15): torn
//! table line, death between table and OUTCOME, OUTCOME over an unsealed table, ENOSPC, I/O error, panic, abort,
//! memory, helpers that die with the child and one that escapes.

#[path = "fake_child_common/mod.rs"]
mod common;
use common::*;
use rustred_campaign_contract::classify::{Accept, KillReason, OutcomeKind, WaitStatus};
use rustred_campaign_contract::events::Event;
use rustred_campaign_contract::fake::*;
use rustred_campaign_contract::seal::{SealVerdict, UnsealReason, verify_sealed};
use rustred_campaign_contract::stop::StopSource;
use rustred_campaign_contract::{ExitCode, StopCode};
use std::time::{Duration, Instant};

const T: Duration = Duration::from_secs(30);

fn base() -> FakeScript {
    script(vec![
        rule_case(0, 0),
        rule_case(1, 0),
        covered_case(2, 0),
        rule_case(3, 0),
        end(None),
    ])
}

fn with_fault(mut sc: FakeScript, at_op: usize, kind: FaultKind) -> FakeScript {
    sc.faults = vec![Fault {
        at_op,
        kind,
        bytes: 23,
        code: 0,
        path: None,
    }];
    sc
}

#[test]
fn f5_torn_table_line_then_resume() {
    // brief 6 / F5: die mid-write of a table line: no OUTCOME, torn last line, RESUME valid -> row 21; the resumed
    // run ends with the uninterrupted canonical table
    let mut root = Root::new("torn");
    let dir0 = root.run_dir("ref", 1);
    let (rr, w) = run_to_end(&mut root, &spec(&dir0, "ref", 1, base()), T);
    assert_eq!(w, WaitStatus::Exited(0));
    let want = canonical_of(&rr);
    let dir = root.run_dir("t", 1);
    let s = spec(&dir, "t", 1, with_fault(base(), 2, FaultKind::TornWrite));
    let (r, w) = run_to_end(&mut root, &s, T);
    assert_eq!(w, WaitStatus::Signaled(libc::SIGKILL));
    assert!(!r.layout.outcome().exists());
    let bytes = std::fs::read(r.layout.table()).unwrap();
    assert_ne!(bytes.last(), Some(&b'\n'), "torn last line");
    let c = r.classify(w, KillReason::None);
    assert_eq!(
        (c.row, c.kind, c.accept),
        (21, OutcomeKind::Lost, Accept::Nothing)
    );
    assert!(c.resume_from_r, "RESUME prefix excludes the torn tail");
    let s2 = resume_spec(&root, &r, 2);
    let (r2, w2) = run_to_end(&mut root, &s2, T);
    assert_eq!(w2, WaitStatus::Exited(0), "{}", r2.stderr());
    assert_eq!(r2.classify(w2, KillReason::None).row, 1);
    assert_eq!(canonical_of(&r2), want);
    assert_eq!(
        std::fs::read(r2.layout.table()).unwrap(),
        std::fs::read(rr.layout.table()).unwrap(),
        "byte-identical without jitter"
    );
    root.assert_no_process_left();
}

#[test]
fn die_after_table_before_outcome() {
    // brief 7: the table is sealed on disk but OUTCOME is missing: nothing accepted -> row 21
    let mut root = Root::new("kbo");
    let dir = root.run_dir("k", 1);
    let s = spec(
        &dir,
        "k",
        1,
        with_fault(base(), 0, FaultKind::KillBeforeOutcome),
    );
    let (r, w) = run_to_end(&mut root, &s, T);
    assert_eq!(w, WaitStatus::Signaled(libc::SIGKILL));
    assert!(!r.layout.outcome().exists());
    let last = std::fs::read_to_string(r.layout.table())
        .unwrap()
        .lines()
        .last()
        .unwrap()
        .to_string();
    assert!(last.contains("\"type\":\"end\""));
    let c = r.classify(w, KillReason::None);
    assert_eq!((c.row, c.accept), (21, Accept::Nothing));
    root.assert_no_process_left();
}

#[test]
fn outcome_over_unsealed_table_is_a_contract_violation() {
    // brief 8: OUTCOME claims a sealed table but the end record is missing -> verifier NoEndRecord -> row 22
    let mut root = Root::new("unsealed");
    let dir = root.run_dir("u", 1);
    let s = spec(
        &dir,
        "u",
        1,
        with_fault(base(), 0, FaultKind::OutcomeUnsealed),
    );
    let (r, w) = run_to_end(&mut root, &s, T);
    assert_eq!(w, WaitStatus::Exited(0));
    let t = r.outcome().job.outputs.table.unwrap();
    assert!(t.sealed);
    assert!(matches!(
        verify_sealed(&r.layout.table(), &t),
        SealVerdict::Unsealed(UnsealReason::NoEndRecord(_))
    ));
    let c = r.classify(w, KillReason::None);
    assert_eq!(
        (c.row, c.kind, c.accept),
        (22, OutcomeKind::Crashed, Accept::Nothing)
    );
    assert!(c.attention);
    root.assert_no_process_left();
}

#[test]
fn f11_enospc_via_dev_full_and_injected_io_error() {
    // brief 15 / F11: ENOSPC (a write to /dev/full): reserve freed, OUTCOME stop nospace, exit 50 -> row 15
    let mut root = Root::new("enospc");
    let dir = root.run_dir("n", 1);
    let mut sc = base();
    sc.faults = vec![Fault {
        at_op: 2,
        kind: FaultKind::Enospc,
        bytes: 0,
        code: 0,
        path: Some("/dev/full".into()),
    }];
    let s = spec(&dir, "n", 1, sc);
    let (r, w) = run_to_end(&mut root, &s, T);
    assert_eq!(w, WaitStatus::Exited(50), "{}", r.stderr());
    let o = r.outcome();
    assert_eq!(o.job.stop.code, Some(StopCode::Nospace));
    assert!(o.job.error.is_some());
    assert!(
        !r.layout.reserve().exists(),
        "reserve deleted before OUTCOME"
    );
    let c = r.classify(w, KillReason::None);
    assert_eq!((c.row, c.kind), (15, OutcomeKind::Lost));
    assert!(c.resume_from_r);
    // injected EIO: exit 51 -> row 16
    let dir = root.run_dir("io", 1);
    let s = spec(&dir, "io", 1, with_fault(base(), 1, FaultKind::IoError));
    let (r, w) = run_to_end(&mut root, &s, T);
    assert_eq!(w, WaitStatus::Exited(51));
    assert_eq!(r.outcome().job.stop.code, Some(StopCode::IoError));
    assert_eq!(r.classify(w, KillReason::None).row, 16);
    root.assert_no_process_left();
}

#[test]
fn f12_panic_and_abort() {
    // F12: panic -> 70 with OUTCOME (row 17); abort -> signal 6 without OUTCOME (row 23)
    let mut root = Root::new("panic");
    let dir = root.run_dir("p", 1);
    let s = spec(&dir, "p", 1, with_fault(base(), 1, FaultKind::Panic));
    let (r, w) = run_to_end(&mut root, &s, T);
    assert_eq!(w, WaitStatus::Exited(70));
    let o = r.outcome();
    assert_eq!(o.job.stop.code, Some(StopCode::Panic));
    assert_eq!(r.classify(w, KillReason::None).row, 17);
    let dir = root.run_dir("a", 1);
    let s = spec(&dir, "a", 1, with_fault(base(), 1, FaultKind::Abort));
    let (r, w) = run_to_end(&mut root, &s, T);
    assert_eq!(w, WaitStatus::Signaled(libc::SIGABRT));
    assert!(!r.layout.outcome().exists());
    let c = r.classify(w, KillReason::None);
    assert_eq!(c.row, 23);
    assert!(c.resume_from_r);
    root.assert_no_process_left();
}

#[test]
fn f6_memory_hard_kill_and_soft_limit() {
    // brief 9 / F6: allocate and touch N MB; the supervisor's memory kill (SIGKILL, K = memory) -> row 18
    let mut root = Root::new("mem");
    let dir = root.run_dir("m", 1);
    let s = spec(
        &dir,
        "m",
        1,
        script(vec![
            rule_case(0, 0),
            FakeOp::Alloc { mb: 64 },
            FakeOp::Heartbeats { n: 30 },
            end(None),
        ]),
    );
    let mut r = spawn(&mut root, &s);
    let t0 = Instant::now();
    loop {
        let kb = std::fs::read_to_string(format!("/proc/{}/statm", r.pid))
            .ok()
            .and_then(|x| x.split_whitespace().nth(1)?.parse::<u64>().ok())
            .unwrap_or(0)
            * 4;
        if kb >= 64 * 1024 {
            break;
        }
        assert!(t0.elapsed() < T, "RSS never reached 64 MiB");
        std::thread::sleep(Duration::from_millis(20));
    }
    r.signal_group(libc::SIGKILL);
    let w = r.wait(T);
    let c = r.classify(w, KillReason::Memory);
    assert_eq!((c.row, c.kind), (18, OutcomeKind::KilledMemory));
    // soft limit 32 MB: the child stops itself gracefully, exit 22 -> row 10
    let dir = root.run_dir("ms", 1);
    let mut s = spec(
        &dir,
        "ms",
        1,
        script(vec![
            rule_case(0, 0),
            FakeOp::Alloc { mb: 64 },
            FakeOp::Heartbeats { n: 30 },
            end(None),
        ]),
    );
    s.budgets.mem_soft_bytes = Some(32 << 20);
    let (r, w) = run_to_end(&mut root, &s, T);
    assert_eq!(w, WaitStatus::Exited(22), "{}", r.stderr());
    let o = r.outcome();
    assert_eq!(
        (o.job.stop.code, o.job.stop.source),
        (Some(StopCode::BudgetMemory), Some(StopSource::Memory))
    );
    assert_eq!(r.classify(w, KillReason::None).row, 10);
    root.assert_no_process_left();
}

fn helper_script(h: HelperPlan) -> FakeScript {
    let mut sc = script(vec![
        rule_case(0, 0),
        FakeOp::Helper { index: 0 },
        FakeOp::Heartbeats { n: 30 },
        end(None),
    ]);
    sc.helpers = vec![h];
    sc
}

#[test]
fn f7_helper_dies_with_the_child() {
    // brief 10 / F7: helper with PDEATHSIG and SIGTERM ignored. (a) SIGKILL of the parent ONLY: the helper dies by
    // PDEATHSIG within 1 s. (b) group SIGTERM: the helper ignores it, the child stops gracefully and kills it.
    let mut root = Root::new("helper");
    let h = HelperPlan {
        sleep_ms: 60_000,
        pdeathsig: true,
        ignore_sigterm: true,
        escape_group: false,
    };
    let dir = root.run_dir("a", 1);
    let s = spec(&dir, "a", 1, helper_script(h));
    let mut r = spawn(&mut root, &s);
    r.wait_event(
        T,
        |e| matches!(e, Event::Engine { event } if event.get("helper").is_some()),
    );
    let (hp, registered) = r.helper_pids()[0];
    assert!(registered && alive(hp));
    assert_eq!(
        stat_of(hp).unwrap().1,
        r.pid,
        "helper stays in the child's group"
    );
    r.signal_pid(libc::SIGKILL);
    let w = r.wait(T);
    assert_eq!(w, WaitStatus::Signaled(libc::SIGKILL));
    let t0 = Instant::now();
    while alive(hp) {
        assert!(
            t0.elapsed() < Duration::from_secs(1),
            "helper survived its parent"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let dir = root.run_dir("b", 1);
    let s = spec(&dir, "b", 1, helper_script(h));
    let mut r = spawn(&mut root, &s);
    r.wait_event(
        T,
        |e| matches!(e, Event::Engine { event } if event.get("helper").is_some()),
    );
    let (hp, _) = r.helper_pids()[0];
    r.signal_group(libc::SIGTERM);
    let w = r.wait(T);
    assert_eq!(w, WaitStatus::Exited(30));
    assert!(
        !alive(hp),
        "the child killed and reaped its helper before exiting"
    );
    assert_eq!(r.classify(w, KillReason::Stop).row, 11);
    root.assert_no_process_left();
}

#[test]
fn f8_escaping_helper_is_detected() {
    // brief 10 / F8: a helper that deliberately escapes into its own session without PDEATHSIG survives the child:
    // it is found (not in the child's group, still alive) and killed by the test, as the supervisor's orphan scan
    // would do
    let mut root = Root::new("escape");
    let h = HelperPlan {
        sleep_ms: 30_000,
        pdeathsig: false,
        ignore_sigterm: true,
        escape_group: true,
    };
    let dir = root.run_dir("e", 1);
    let mut sc = helper_script(h);
    sc.script[2] = FakeOp::Heartbeats { n: 1 };
    let s = spec(&dir, "e", 1, sc);
    let mut r = spawn(&mut root, &s);
    let w = r.wait(T);
    assert_eq!(w, WaitStatus::Exited(0));
    let (hp, registered) = r.helper_pids()[0];
    root.track_pid(hp);
    assert!(!registered);
    let st = stat_of(hp).expect("escaped helper still alive after the child ended");
    assert_ne!(st.1, r.pid, "escaped from the child's process group");
    assert_eq!(st.1, hp, "own group (setsid)");
    // the orphan scan: SIGKILL it (it is ours: pid from our child's events) and make sure it is gone
    // SAFETY: a process spawned by this test's child.
    unsafe { libc::kill(hp, libc::SIGKILL) };
    let t0 = Instant::now();
    while alive(hp) {
        assert!(t0.elapsed() < Duration::from_secs(2));
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(r.classify(w, KillReason::None).row, 1);
    root.assert_no_process_left();
}

#[test]
fn exit_code_and_status_consistency_of_every_outcome() {
    // every OUTCOME the fake writes passes the contract's own consistency check (exit code, status, stop)
    let mut root = Root::new("consist");
    for (k, code) in [ExitCode::Usage, ExitCode::Internal]
        .into_iter()
        .enumerate()
    {
        let job = format!("c{k}");
        let dir = root.run_dir(&job, 1);
        let s = spec(
            &dir,
            &job,
            1,
            script(vec![FakeOp::Fail {
                code,
                message: "m".into(),
            }]),
        );
        let (r, _) = run_to_end(&mut root, &s, T);
        r.outcome().job.check().unwrap();
    }
    root.assert_no_process_left();
}
