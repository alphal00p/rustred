//! Campaign lock: refusal while the owner lives, takeover (with a ledger record) once it is provably dead,
//! including a crashed supervisor whose child is still alive (the lock fd is NOT inherited, so the child
//! does not block the takeover; it is reported as an orphan through the token).
//!
//! The "supervisor" is this test binary re-executed with `RR_LOCK_HELPER=<dir>`: it takes the lock, spawns
//! `sleep` with the token in its environment, prints `ready <child pid>` and waits to be killed.
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code
mod ledger_support;

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use ledger_support::*;
use rustred_campaign::clock::{Clock, SystemClock};
use rustred_campaign::ledger::{Event, LedgerConfig};
use rustred_campaign::lock::{self, DeadReason, LockError, LockOutcome};

/// Helper entry point (runs only in the re-executed child).
/// procfix7 round 5 (gate9 m9-lockflake: 7 / 40 failures with the binary's tests in parallel, 0 / 40 alone): a test that
/// forks (the helper supervisor, its `sleep`) duplicates EVERY descriptor of this process into the child until the
/// child's exec closes it, the lock file of a concurrent in-process test included, and a duplicated descriptor keeps that
/// test's flock alive after its `release()` (`Busy { owner: None }`). The tests of this binary therefore run one at a
/// time; each then guarantees exactly what it states, with no descriptor of another test in flight.
fn serial() -> std::sync::MutexGuard<'static, ()> {
    static M: std::sync::Mutex<()> = std::sync::Mutex::new(());
    M.lock().unwrap_or_else(|p| p.into_inner())
}

#[test]
#[allow(clippy::zombie_processes)] // the orphan is the point of the test
fn zz_lock_helper_process() {
    let Some(dir) = std::env::var_os("RR_LOCK_HELPER") else {
        return;
    };
    let clk = SystemClock::new();
    let (lk, _) = lock::acquire(
        std::path::Path::new(&dir),
        &clk,
        &["helper-supervisor".into()],
    )
    .expect("helper takes the lock");
    let child = Command::new("sleep")
        .arg("120")
        .env(lock::TOKEN_ENV, lk.token())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    // direct stdout write (println! may be captured by the test harness)
    {
        use std::io::Write;
        let mut o = std::io::stdout().lock();
        writeln!(o, "ready {}", child.id()).unwrap();
        o.flush().unwrap();
    }
    std::thread::sleep(Duration::from_secs(120));
    drop(lk);
}

#[allow(clippy::zombie_processes)] // the caller kills and reaps it
fn spawn_helper(dir: &std::path::Path) -> (std::process::Child, u32) {
    let exe = std::env::current_exe().unwrap();
    let mut c = Command::new(exe)
        .args([
            "--exact",
            "zz_lock_helper_process",
            "--nocapture",
            "--test-threads",
            "1",
        ])
        .env("RR_LOCK_HELPER", dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut r = BufReader::new(c.stdout.take().unwrap());
    let t0 = Instant::now();
    let mut line = String::new();
    loop {
        line.clear();
        assert!(r.read_line(&mut line).unwrap() > 0, "helper exited early");
        // the harness may print "test <name> ... " on the same line first
        if let Some(p) = line
            .find("ready ")
            .map(|i| &line.trim()[i..])
            .and_then(|t| t.strip_prefix("ready "))
        {
            break (c, p.parse().unwrap());
        }
        assert!(
            t0.elapsed() < Duration::from_secs(60),
            "helper did not become ready"
        );
    }
}

fn kill(pid: u32, sig: i32) {
    // SAFETY: our own child processes only.
    unsafe {
        libc::kill(pid as i32, sig);
    }
}

#[test]
fn second_lock_in_process_is_refused_and_release_allows_fresh() {
    let _serial = serial();
    let d = scratch("lock1");
    let _c = Cleanup(d.clone());
    let clk = SystemClock::new();
    let (lk, out) = lock::acquire(&d, &clk, &["a".into()]).unwrap();
    assert!(matches!(out, LockOutcome::Fresh));
    let owner = lock::read_owner(&d).unwrap().unwrap();
    assert_eq!(owner.identity.pid, std::process::id());
    assert_eq!(owner.token.len(), 32);
    match lock::acquire(&d, &clk, &["b".into()]) {
        Err(LockError::HeldByLiveOwner { owner }) => assert_eq!(owner.argv, vec!["a".to_string()]),
        other => panic!("expected HeldByLiveOwner, got {other:?}"),
    }
    lk.release().unwrap();
    assert!(lock::read_owner(&d).unwrap().is_none());
    let (_lk, out) = lock::acquire(&d, &clk, &["c".into()]).unwrap();
    assert!(matches!(out, LockOutcome::Fresh));
}

#[test]
fn stale_owner_records_are_taken_over() {
    let _serial = serial();
    let d = scratch("lock2");
    let _c = Cleanup(d.clone());
    let clk = SystemClock::new();
    let me = lock::current_identity().unwrap();
    let write_owner = |id: lock::ProcessIdentity| {
        let rec = lock::OwnerRecord {
            schema: lock::LOCK_SCHEMA.into(),
            identity: id,
            argv: vec!["old".into()],
            started_utc: "2026-11-13T00:00:00.000Z".into(),
            token: "feedfeedfeedfeedfeedfeedfeedfeed".into(),
        };
        rustred_campaign::atomic::write_json_atomic(&d.join(lock::OWNER_FILE), &rec).unwrap();
    };
    // pid reused (start time differs)
    write_owner(lock::ProcessIdentity {
        start_ticks: me.start_ticks + 12345,
        ..me.clone()
    });
    let (lk, out) = lock::acquire(&d, &clk, &["new".into()]).unwrap();
    assert!(
        matches!(
            out,
            LockOutcome::TookOver {
                reason: DeadReason::StartTimeDiffers { .. },
                ..
            }
        ),
        "{out:?}"
    );
    drop(lk);
    // reboot (boot id differs)
    write_owner(lock::ProcessIdentity {
        boot_id: "00000000-0000-0000-0000-000000000000".into(),
        ..me.clone()
    });
    let (lk, out) = lock::acquire(&d, &clk, &["new".into()]).unwrap();
    assert!(matches!(
        out,
        LockOutcome::TookOver {
            reason: DeadReason::BootIdDiffers { .. },
            ..
        }
    ));
    drop(lk);
    // pid absent
    write_owner(lock::ProcessIdentity {
        pid: u32::MAX - 3,
        ..me.clone()
    });
    let (lk, out) = lock::acquire(&d, &clk, &["new".into()]).unwrap();
    assert!(matches!(
        out,
        LockOutcome::TookOver {
            reason: DeadReason::PidAbsent,
            ..
        }
    ));
    drop(lk);
    // a renamed host (audit A m6): judged by boot id + pid + start ticks, never refused forever
    write_owner(lock::ProcessIdentity {
        host: "elsewhere.invalid".into(),
        pid: u32::MAX - 3,
        ..me
    });
    let (lk, out) = lock::acquire(&d, &clk, &["new".into()]).unwrap();
    assert!(matches!(
        out,
        LockOutcome::TookOver {
            reason: DeadReason::PidAbsent,
            ..
        }
    ));
    drop(lk);
}

#[test]
fn crashed_supervisor_with_live_child_is_taken_over_and_recorded() {
    let _serial = serial();
    let d = scratch("lock3");
    let _c = Cleanup(d.clone());
    let (mut helper, child_pid) = spawn_helper(&d);
    let clk = SystemClock::new();
    // owner alive: refuse, naming it
    match lock::acquire(&d, &clk, &["successor".into()]) {
        Err(LockError::HeldByLiveOwner { owner }) => assert_eq!(owner.identity.pid, helper.id()),
        other => panic!("expected HeldByLiveOwner, got {other:?}"),
    }
    let token = lock::read_owner(&d).unwrap().unwrap().token;
    // gate8 (i): std's spawn returns inside execve, before the new image's environment bounds are set (its environ reads
    // EMPTY for a moment, 2,754 of 3,000 spawns in tests/gate8_flake.rs): wait until the child's environ is readable
    // (bounded) before judging the token scan.
    let t0 = Instant::now();
    while !std::fs::read(format!("/proc/{child_pid}/environ")).is_ok_and(|b| !b.is_empty()) {
        assert!(
            t0.elapsed() < Duration::from_secs(10),
            "the child's environ never became readable"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        lock::scan_token_holders(&token).unwrap().holders,
        vec![child_pid],
        "the child carries the token"
    );
    // the child does NOT hold the lock file open (no inherited descriptor)
    assert!(!lock::holders_of(&d.join(lock::LOCK_FILE)).contains(&child_pid));
    // hard kill of the supervisor (its child lives on)
    kill(helper.id(), libc::SIGKILL);
    helper.wait().unwrap();
    assert!(lock::start_ticks(child_pid).is_ok(), "child still alive");
    let (lk, out) =
        lock::acquire(&d, &clk, &["successor".into()]).expect("takeover despite the live child");
    let LockOutcome::TookOver {
        previous,
        reason,
        orphans,
    } = out
    else {
        panic!("expected takeover")
    };
    assert_eq!(previous.identity.pid, helper.id());
    assert_eq!(reason, DeadReason::PidAbsent);
    assert_eq!(
        orphans,
        vec![child_pid],
        "orphan reported for the process layer"
    );
    // the takeover goes into the ledger
    let ck: Arc<dyn Clock> = clock();
    let mut l =
        rustred_campaign::ledger::Ledger::create(&d, &lk, ck, LedgerConfig::default(), genesis())
            .unwrap();
    l.append(Event::LockTakeover {
        previous,
        reason,
        orphans: orphans.clone(),
    })
    .unwrap();
    assert_eq!(l.state().takeovers, 1);
    drop(l);
    drop(lk);
    kill(child_pid, libc::SIGKILL);
    for _ in 0..100 {
        if lock::start_ticks(child_pid).is_err()
            || std::fs::read_to_string(format!("/proc/{child_pid}/stat"))
                .is_ok_and(|s| s.contains(") Z "))
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
