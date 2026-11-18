//! gate17 REVIEW test (not product code): the kernel facts behind round 15's `LaunchState::Gone` (launch/real.rs
//! `launch_state`: the cgroup read first, the exe read second; a read that says Gone returns `LaunchState::Gone`, and
//! supervisor/mod.rs:5182 then keeps `pending_since` and the 10-s window as they were).
//!
//! `Gone` was meant for a launcher that ENDED during the look (an unreaped zombie: no exe link). The same reads also
//! answer that way for a LIVE process whose thread-group LEADER thread has ended (main thread gone by the raw exit
//! syscall, a worker thread still running): its exe link reads Gone at EVERY look, its cgroup reads, and waitid reports
//! nothing (the group lives), so `poll_exit` never sees an end. A job program in that state, after a launcher that was
//! classed pending, keeps `pending_since` at every look and the 10-s rule never fires for it (the manager gate bounds it
//! instead: warn time, then the wait limit, exit 69).
//!
//! Spawns ONE child (python3, our own); run ONLY inside a private pid namespace (`LIGHT_PIDNS=1 light_build.sh ...`,
//! rule L10). Outside a namespace it returns at once.
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

use rustred_campaign::resources::{ProcError, procfs};
use std::time::{Duration, Instant};

#[cfg(target_arch = "x86_64")]
#[test]
fn a_live_process_whose_leader_thread_ended_reads_gone_at_every_look_and_reports_no_exit() {
    if !rustred_campaign::launch::in_private_namespace() {
        return;
    }
    // 60 = SYS_exit on x86_64: ends the calling (main) thread only; the worker lives on (60 s)
    let src = "import ctypes, threading, time\n\
               t = threading.Thread(target=time.sleep, args=(60,)); t.start()\n\
               ctypes.CDLL(None).syscall(60, 0)\n";
    let mut c = std::process::Command::new("python3")
        .args(["-I", "-c", src])
        .spawn()
        .unwrap();
    let pid = c.id();
    let root = procfs::sys();
    let t0 = Instant::now();
    loop {
        let st = root.status(pid).unwrap();
        if st.state().unwrap() == 'Z' && st.threads().unwrap() >= 2 {
            break;
        }
        assert!(
            t0.elapsed() < Duration::from_secs(20),
            "never a zombie leader with a live worker"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    for look in 0..5 {
        // the group lives: the kernel counts the worker
        let st = root.status(pid).unwrap();
        assert!(
            st.threads().unwrap() >= 2,
            "look {look}: the worker is gone"
        );
        // launch_state's FIRST read: the cgroup line is there (the leader's record)
        let cg = root.cgroup_text(pid, Duration::from_secs(5)).unwrap();
        assert!(
            cg.lines().any(|l| l.starts_with("0::")),
            "look {look}: {cg}"
        );
        // launch_state's SECOND read: Gone -> LaunchState::Gone (real.rs:2116-2118)
        assert_eq!(root.exe(pid), Err(ProcError::Gone), "look {look}");
        // poll_exit's peek (waitid WEXITED | WNOHANG | WNOWAIT): nothing to report
        // SAFETY: waitid on our own child with a zeroed siginfo out-parameter.
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        let r = unsafe {
            libc::waitid(
                libc::P_PID,
                pid,
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        assert_eq!(r, 0, "look {look}: waitid failed");
        // SAFETY: reading the pid field of the siginfo the kernel filled (0 = no state change).
        assert_eq!(
            unsafe { info.si_pid() },
            0,
            "look {look}: an exit was reported"
        );
        std::thread::sleep(Duration::from_millis(500));
    }
    let _ = c.kill();
    let _ = c.wait();
}
