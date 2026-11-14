//! Stop-counter semantics with real signals delivered to this test process (own binary: handlers are process-wide).

#[path = "proc_common/mod.rs"]
mod common;

use common::wait_until;
use rustred_campaign::signals::{StopLevel, StopSignals};
use std::time::Duration;

fn raise(sig: i32) {
    // SAFETY: kill(getpid(), sig) with a signal whose handler is installed.
    assert_eq!(unsafe { libc::kill(libc::getpid(), sig) }, 0);
}

#[test]
fn stop_counter_drain_then_force_winch_and_hangup() {
    let s = StopSignals::install().unwrap();
    let h = s.handle();
    assert_eq!(h.level(), StopLevel::Running);
    raise(libc::SIGINT);
    wait_until("first SIGINT counted", Duration::from_secs(30), || {
        h.count() == 1
    });
    assert_eq!(h.level(), StopLevel::Drain);
    raise(libc::SIGTERM);
    wait_until("SIGTERM counted", Duration::from_secs(30), || {
        h.count() == 2
    });
    assert_eq!(h.level(), StopLevel::Force);
    // The dashboard's key path bumps the same counter.
    assert_eq!(h.request_stop(), StopLevel::Force);
    assert_eq!(h.count(), 3);
    raise(libc::SIGWINCH);
    wait_until("SIGWINCH flag", Duration::from_secs(30), || h.take_winch());
    assert!(!h.take_winch(), "the flag is consumed once");
    assert!(!h.hangup_seen());
    raise(libc::SIGHUP);
    // SIGHUP never escalates (audit A m10): the flag is set, the counter stays.
    wait_until("SIGHUP", Duration::from_secs(30), || h.hangup_seen());
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(h.count(), 3);
    drop(s);
    // After unregistering, signal-hook keeps the signals caught and ignored: the process survives a late SIGTERM.
    raise(libc::SIGTERM);
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(h.count(), 3);
}
