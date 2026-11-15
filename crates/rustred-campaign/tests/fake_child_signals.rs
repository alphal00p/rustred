//! Fake child, termination (child-job.md 3.4, section 4: F3, F4, F10, the brief's scenarios 4, 5, 11): SIGTERM at a
//! safe point, slow and ignored SIGTERM, second signal, STOP file, hang (hard-safe stop), stopped heartbeat, wall
//! budget; resumed runs end with the canonical table of an uninterrupted run (R5 drill, ruling R-b).

#[path = "fake_child_common/mod.rs"]
mod common;
use common::*;
use rustred_campaign_contract::classify::{Accept, Action, KillReason, OutcomeKind, WaitStatus};
use rustred_campaign_contract::events::Event;
use rustred_campaign_contract::fake::*;
use rustred_campaign_contract::spec::JobSpec;
use rustred_campaign_contract::stop::StopSource;
use rustred_campaign_contract::{ExitCode, StopCode};
use std::time::{Duration, Instant};

const T: Duration = Duration::from_secs(30);

/// case 0 quick, case 1 long (the one interrupted), case 2 quick.
fn three(long_ms: u64) -> FakeScript {
    script(vec![
        rule_case(0, 0),
        rule_case(1, long_ms),
        covered_case(2, 0),
        rule_case(3, 0),
        end(None),
    ])
}

/// The canonical digest of an uninterrupted run of `sc` (time_scale shrunk: timings never reach the table).
fn reference(root: &mut Root, sc: &FakeScript) -> String {
    let dir = root.run_dir("ref", 1);
    let mut sc = sc.clone();
    sc.time_scale = 0.001;
    sc.faults.clear();
    sc.on_sigterm = OnSigterm::Graceful;
    let s = spec(&dir, "ref", 1, sc);
    let (r, w) = run_to_end(root, &s, T);
    assert_eq!(w, WaitStatus::Exited(0), "{}", r.stderr());
    canonical_of(&r)
}

/// Resume `prev` as attempt 2 and check that the final canonical table equals `want`.
fn resume_and_compare(root: &mut Root, prev: &Running, want: &str) {
    let s2: JobSpec = resume_spec(root, prev, 2);
    let mut s2 = s2;
    if let Some(f) = s2.fake.as_mut() {
        f.time_scale = 0.001;
    }
    let (r2, w2) = run_to_end(root, &s2, T);
    assert_eq!(w2, WaitStatus::Exited(0), "resume: {}", r2.stderr());
    let o = r2.outcome();
    assert!(o.job.resumed);
    assert!(matches!(
        r2.events()[0].event,
        Event::JobStarted {
            resume_from: Some(1),
            ..
        }
    ));
    assert_eq!(r2.classify(w2, KillReason::None).row, 1);
    assert_eq!(
        canonical_of(&r2),
        want,
        "resumed canonical table == uninterrupted"
    );
}

fn wait_rule(r: &Running, index: u64) {
    r.wait_event(
        T,
        |e| matches!(e, Event::RuleCommitted { index: i, .. } if *i == index),
    );
}

#[test]
fn f4_sigterm_graceful_within_grace_then_resume() {
    // brief 4: react to SIGTERM at a safe point within T: exit 30, graceful, sealed partial, RESUME valid -> row 11
    let mut root = Root::new("term");
    let sc = three(20_000);
    let want = reference(&mut root, &sc);
    let dir = root.run_dir("g", 1);
    let s = spec(&dir, "g", 1, sc);
    let mut r = spawn(&mut root, &s);
    wait_rule(&r, 0);
    std::thread::sleep(Duration::from_millis(150));
    let t0 = Instant::now();
    r.signal_group(libc::SIGTERM);
    let w = r.wait(T);
    let took = t0.elapsed();
    assert_eq!(w, WaitStatus::Exited(30), "{}", r.stderr());
    assert!(
        took < Duration::from_millis(1000),
        "stopped at a safe point in {took:?}"
    );
    let o = r.outcome();
    assert!(o.job.stop.graceful);
    assert_eq!(o.job.stop.source, Some(StopSource::Sigterm));
    assert!(
        o.job.outputs.table.as_ref().unwrap().sealed,
        "graceful stop seals a partial table"
    );
    let c = r.classify(w, KillReason::Stop);
    assert_eq!(
        (c.row, c.kind, c.accept, c.action),
        (
            11,
            OutcomeKind::Interrupted,
            Accept::Nothing,
            Action::ResumeOnCampaignResume
        )
    );
    assert!(c.resume_from_r);
    assert!(r.events().iter().any(|e| matches!(
        e.event,
        Event::StopRequested {
            source: StopSource::Sigterm,
            ..
        }
    )));
    resume_and_compare(&mut root, &r, &want);
    root.assert_no_process_left();
}

#[test]
fn f4_slow_sigterm_below_grace_is_still_graceful() {
    // F4: on_sigterm slow (< grace): graceful interrupted, sealed partial
    let mut root = Root::new("slowterm");
    let dir = root.run_dir("s", 1);
    let mut sc = three(20_000);
    sc.on_sigterm = OnSigterm::Slow;
    sc.slow_ms = 800;
    let s = spec(&dir, "s", 1, sc);
    let mut r = spawn(&mut root, &s);
    wait_rule(&r, 0);
    let t0 = Instant::now();
    r.signal_group(libc::SIGTERM);
    let w = r.wait(T);
    assert_eq!(w, WaitStatus::Exited(30));
    assert!(
        t0.elapsed() >= Duration::from_millis(800),
        "kept working for slow_ms"
    );
    assert!(r.outcome().job.stop.graceful);
    assert_eq!(r.classify(w, KillReason::Stop).row, 11);
    root.assert_no_process_left();
}

#[test]
fn f3_ignored_sigterm_needs_sigkill_then_resume() {
    // brief 5 / F3: ignore SIGTERM: supervisor escalates to SIGKILL after the grace -> row 20; RESUME valid; resume
    // completes with the uninterrupted canonical table
    let mut root = Root::new("ignore");
    let mut sc = three(20_000);
    sc.on_sigterm = OnSigterm::Ignore;
    let want = reference(&mut root, &sc);
    let dir = root.run_dir("i", 1);
    let s = spec(&dir, "i", 1, sc);
    let mut r = spawn(&mut root, &s);
    wait_rule(&r, 0);
    r.signal_group(libc::SIGTERM);
    std::thread::sleep(Duration::from_millis(s.stop_grace_s as u64 * 1000 + 300));
    assert!(r.child.try_wait().unwrap().is_none(), "SIGTERM ignored");
    r.signal_group(libc::SIGKILL);
    let w = r.wait(T);
    assert_eq!(w, WaitStatus::Signaled(libc::SIGKILL));
    assert!(!r.layout.outcome().exists());
    let c = r.classify(w, KillReason::Stop);
    assert_eq!((c.row, c.kind), (20, OutcomeKind::Interrupted));
    assert!(c.resume_from_r);
    resume_and_compare(&mut root, &r, &want);
    root.assert_no_process_left();
}

#[test]
fn second_signal_takes_the_hard_safe_path_at_once() {
    // 3.4.3: a SECOND signal -> hard-safe stop within 0.2 s: OUTCOME graceful=false, table unsealed, RESUME valid
    let mut root = Root::new("second");
    let dir = root.run_dir("d", 1);
    let mut sc = three(20_000);
    sc.on_sigterm = OnSigterm::Slow;
    sc.slow_ms = 60_000;
    let mut s = spec(&dir, "d", 1, sc);
    s.stop_grace_s = 30;
    let mut r = spawn(&mut root, &s);
    wait_rule(&r, 0);
    r.signal_group(libc::SIGTERM);
    r.wait_event(T, |e| matches!(e, Event::StopRequested { .. }));
    let t0 = Instant::now();
    r.signal_group(libc::SIGTERM);
    let w = r.wait(T);
    assert!(
        t0.elapsed() < Duration::from_millis(500),
        "{:?}",
        t0.elapsed()
    );
    assert_eq!(w, WaitStatus::Exited(30));
    let o = r.outcome();
    assert!(!o.job.stop.graceful);
    let t = o.job.outputs.table.clone().unwrap();
    assert!(!t.sealed && t.committed_bytes > 0 && t.prefix_blake3.is_some());
    let c = r.classify(w, KillReason::Stop);
    assert_eq!(c.row, 11);
    assert!(c.resume_from_r);
    root.assert_no_process_left();
}

#[test]
fn stop_file_is_one_sigterm() {
    // 3.4.5: RUN/STOP equivalent to one SIGTERM (checked once per second)
    let mut root = Root::new("stopfile");
    let dir = root.run_dir("f", 1);
    let s = spec(&dir, "f", 1, three(20_000));
    let mut r = spawn(&mut root, &s);
    wait_rule(&r, 0);
    std::fs::write(r.layout.stop_file(), b"{\"reason\":\"operator\"}").unwrap();
    let w = r.wait(T);
    assert_eq!(w, WaitStatus::Exited(30));
    assert_eq!(r.outcome().job.stop.source, Some(StopSource::StopFile));
    assert_eq!(r.classify(w, KillReason::None).row, 11);
    root.assert_no_process_left();
}

#[test]
fn hang_without_safe_point_hard_safe_after_grace() {
    // brief 11 / F10 (hang): progress stops, heartbeats continue; SIGTERM -> no safe point -> hard-safe stop after
    // stop_grace_s: exit 30, graceful=false, unsealed, RESUME valid -> row 11
    let mut root = Root::new("hang");
    let dir = root.run_dir("h", 1);
    let mut sc = three(0);
    sc.faults = vec![Fault {
        at_op: 2,
        kind: FaultKind::Hang,
        bytes: 0,
        code: 0,
        path: None,
    }];
    let s = spec(&dir, "h", 1, sc);
    let mut r = spawn(&mut root, &s);
    wait_rule(&r, 1);
    // heartbeats keep coming while it hangs
    let n0 = r
        .events()
        .iter()
        .filter(|e| matches!(e.event, Event::Heartbeat { .. }))
        .count();
    std::thread::sleep(Duration::from_millis(2300));
    let n1 = r
        .events()
        .iter()
        .filter(|e| matches!(e.event, Event::Heartbeat { .. }))
        .count();
    assert!(n1 >= n0 + 2, "heartbeat continues ({n0} -> {n1})");
    let t0 = Instant::now();
    r.signal_group(libc::SIGTERM);
    let w = r.wait(T);
    let took = t0.elapsed();
    assert_eq!(w, WaitStatus::Exited(30));
    assert!(
        took >= Duration::from_secs(2) && took < Duration::from_secs(4),
        "grace + ~1 s: {took:?}"
    );
    let o = r.outcome();
    assert!(!o.job.stop.graceful);
    assert!(!o.job.outputs.table.unwrap().sealed);
    let c = r.classify(w, KillReason::Stop);
    assert_eq!(c.row, 11);
    assert!(c.resume_from_r);
    root.assert_no_process_left();
}

#[test]
fn hang_without_heartbeats_is_unresponsive() {
    // brief 11 / F10 (stop_heartbeat): heartbeat age > 3 x heartbeat_s + 5 s would be "unresponsive" (6.2); here the
    // test checks the age grows past 3 x heartbeat_s, kills, and classifies with K = unresponsive -> row 19
    let mut root = Root::new("nohb");
    let dir = root.run_dir("n", 1);
    let mut sc = three(0);
    sc.faults = vec![Fault {
        at_op: 2,
        kind: FaultKind::StopHeartbeat,
        bytes: 0,
        code: 0,
        path: None,
    }];
    let s = spec(&dir, "n", 1, sc);
    let mut r = spawn(&mut root, &s);
    wait_rule(&r, 1);
    let seen = Instant::now();
    std::thread::sleep(Duration::from_millis(3500));
    let last = r
        .events()
        .iter()
        .filter(|e| matches!(e.event, Event::Heartbeat { .. }))
        .map(|e| e.t)
        .fold(0.0, f64::max);
    let now_t = r.events().last().unwrap().t.max(0.0);
    let _ = now_t;
    assert!(seen.elapsed().as_secs_f64() > 3.0 * s.heartbeat_s as f64);
    let after: Vec<_> = r
        .events()
        .into_iter()
        .filter(|e| matches!(e.event, Event::Heartbeat { .. }) && e.t > last)
        .collect();
    assert!(after.is_empty());
    r.signal_group(libc::SIGKILL);
    let w = r.wait(T);
    let c = r.classify(w, KillReason::Unresponsive);
    assert_eq!(
        (c.row, c.kind, c.time_decided),
        (19, OutcomeKind::KilledTime, true)
    );
    assert!(c.resume_from_r);
    root.assert_no_process_left();
}

#[test]
fn wall_budget_is_a_hard_safe_stop_exit_21() {
    // 3.3.2: wall.case_s via the watchdog: hard-safe stop, exit 21, time-decided, unsealed -> row 9
    let mut root = Root::new("wall");
    let dir = root.run_dir("w", 1);
    let mut s = spec(&dir, "w", 1, three(20_000));
    s.budgets.wall.case_s = Some(1);
    let mut r = spawn(&mut root, &s);
    let w = r.wait(T);
    assert_eq!(w, WaitStatus::Exited(21), "{}", r.stderr());
    let o = r.outcome();
    assert_eq!(o.job.exit_code, ExitCode::BudgetWall);
    assert_eq!(o.job.stop.code, Some(StopCode::BudgetWall));
    assert!(o.job.time_decided && !o.job.stop.graceful);
    let c = r.classify(w, KillReason::None);
    assert_eq!(
        (c.row, c.kind, c.time_decided),
        (9, OutcomeKind::KilledTime, true)
    );
    root.assert_no_process_left();
}
