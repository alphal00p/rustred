//! Real-process tests of the launcher: group/session kill, grace escalation, escapees, identity rule, pinning, CLOEXEC,
//! leftovers after a normal exit, reattach after a "restart". Every test asserts that no process of it survives.

#[path = "proc_common/mod.rs"]
mod common;

use common::*;
use rustred_campaign::launch::{
    ChildHandle, ChildLauncher, ExitState, KillReason, LostReason, RealLauncher, ReattachChoice,
    Reattached, peek_exit,
};
use rustred_campaign::resources::{Liveness, ProcessIdentity, affinity_of};
use std::os::fd::AsRawFd;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[test]
fn group_kill_reaches_forked_grandchildren_in_several_groups() {
    let _g = serial();
    let dir = TempDir::new("group");
    let mut l = launcher();
    // Two plain background sleeps (leader's group), a nested shell, and a job-control job in its OWN group.
    let script = "sleep 600 & sh -c 'sleep 600; :' & set -m; sleep 600 & wait";
    let mut h = l.launch(&sh(dir.path(), "group", script)).unwrap();
    let sid = h.identity().pid;
    let s = Arc::clone(l.sampler());
    wait_until("5 members in 2 groups", LONG, || {
        h.poll().unwrap();
        s.latest()
            .and_then(|x| x.session(sid).cloned())
            .is_some_and(|u| u.members.len() >= 5 && u.groups.len() >= 2)
    });
    h.terminate(Duration::from_secs(10), KillReason::Stop);
    let r = drive(&mut h);
    assert!(r.exit.is_some());
    assert!(r.session_verified_empty, "{r:?}");
    assert_eq!(r.kill_reason, Some(KillReason::Stop));
    assert!(r.escaped.is_empty());
    assert_session_gone(sid);
}

#[test]
fn child_ignoring_sigterm_is_killed_after_grace() {
    let _g = serial();
    let dir = TempDir::new("ignore");
    let mut l = launcher();
    let mut h = l
        .launch(&sh(
            dir.path(),
            "ignore",
            "trap '' TERM; sleep 600 & wait; wait",
        ))
        .unwrap();
    let sid = h.identity().pid;
    let s = Arc::clone(l.sampler());
    wait_until("2 members", LONG, || {
        h.poll().unwrap();
        s.latest()
            .and_then(|x| x.session(sid).cloned())
            .is_some_and(|u| u.members.len() >= 2)
    });
    let t0 = Instant::now();
    h.terminate(Duration::from_secs(2), KillReason::Stop);
    let r = drive(&mut h);
    assert!(
        t0.elapsed() >= Duration::from_secs(2),
        "SIGKILL must wait for the grace period"
    );
    assert!(r.sigterm_sent && r.sigkill_sent);
    assert_eq!(r.exit.and_then(|e| e.signal()), Some(libc::SIGKILL));
    assert!(r.session_verified_empty);
    assert_session_gone(sid);
}

/// Since the procfix lane: an escapee that carries the attempt's campaign marker is provably ours; it is reported AND
/// killed through the identity checklist (it was "reported, never chased" before the marker existed).
#[test]
fn escapee_with_marker_is_reported_and_killed() {
    let _g = serial();
    let dir = TempDir::new("escape");
    let mut l = launcher();
    let mut h = l
        .launch(&sh(
            dir.path(),
            "escape",
            "setsid sleep 600 & sleep 600 & wait",
        ))
        .unwrap();
    let sid = h.identity().pid;
    wait_until("escapee seen", LONG, || {
        h.poll().unwrap();
        h.usage().is_some_and(|u| !u.escapees.is_empty())
    });
    h.terminate(Duration::from_secs(5), KillReason::Stop);
    let r = drive(&mut h);
    assert!(r.session_verified_empty, "{r:?}");
    assert_eq!(r.escaped.len(), 1, "{r:?}");
    let e = &r.escaped[0];
    assert_eq!(e.comm, "sleep");
    // It got the SIGTERM of terminate() (or the leftover SIGKILL): it is gone, not alive.
    assert!(!e.alive, "{r:?}");
    assert_session_gone(sid);
    let id = ProcessIdentity {
        pid: e.pid,
        start_ticks: e.start_ticks,
        boot_id: rustred_campaign::resources::boot_id().unwrap(),
    };
    wait_until("escapee gone", Duration::from_secs(60), || {
        !matches!(id.check(), Ok(Liveness::Alive { zombie: false }))
    });
}

#[test]
fn identity_check_refuses_a_recycled_pid() {
    let _g = serial();
    let dir = TempDir::new("identity");
    let mut l = launcher();
    let mut h = l
        .launch(&sh(dir.path(), "identity", "exec sleep 600"))
        .unwrap();
    let real = h.identity().clone();
    let recycled = ProcessIdentity {
        start_ticks: real.start_ticks + 1,
        ..real.clone()
    };
    let marker = rustred_campaign::resources::CampaignMarker::new(
        rustred_campaign::launch::adhoc_campaign_id(),
        "identity",
    )
    .unwrap();
    assert!(matches!(
        rustred_campaign::launch::signal_verified_process(&recycled, &marker, libc::SIGKILL),
        rustred_campaign::launch::Verdict::NotOurs(
            rustred_campaign::launch::Refusal::StartMismatch { .. }
        )
    ));
    // A second launcher (a restarted supervisor) refuses to adopt it too: the pid leads its own session.
    let mut other = launcher();
    assert!(matches!(
        other
            .reattach(
                &recycled,
                "identity",
                ReattachChoice::Kill {
                    grace: Duration::ZERO
                }
            )
            .unwrap(),
        Reattached::Lost(LostReason::Recycled)
    ));
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(
        h.poll_exit().unwrap(),
        None,
        "the refused signal must not have been sent"
    );
    assert_eq!(peek_exit(real.pid).unwrap(), None);
    h.terminate(Duration::from_secs(5), KillReason::Stop);
    let r = drive(&mut h);
    assert!(r.session_verified_empty);
    assert_session_gone(real.pid);
}

#[test]
fn cpu_pinning_is_effective() {
    let _g = serial();
    let dir = TempDir::new("pin");
    let cpu = usable_cpu();
    let mut l = launcher();
    let mut spec = sh(dir.path(), "pin", "sleep 600 & wait");
    spec.cpus = vec![cpu];
    let mut h = l.launch(&spec).unwrap();
    let sid = h.identity().pid;
    assert_eq!(
        affinity_of(sid).unwrap().into_iter().collect::<Vec<_>>(),
        vec![cpu]
    );
    // The forked grandchild inherits the pinning.
    let s = Arc::clone(l.sampler());
    wait_until("grandchild", LONG, || {
        h.poll().unwrap();
        s.latest()
            .and_then(|x| x.session(sid).cloned())
            .is_some_and(|u| u.members.len() >= 2)
    });
    let snap = s.latest().unwrap();
    for m in &snap.session(sid).unwrap().members {
        if let Ok(a) = affinity_of(m.pid) {
            assert_eq!(
                a.into_iter().collect::<Vec<_>>(),
                vec![cpu],
                "member {} ({})",
                m.pid,
                m.comm
            );
        }
    }
    h.terminate(Duration::from_secs(5), KillReason::Stop);
    assert!(drive(&mut h).session_verified_empty);
    assert_session_gone(sid);
}

#[test]
fn child_sees_no_inherited_fd_beyond_stdio_unless_asked() {
    let _g = serial();
    let dir = TempDir::new("cloexec");
    // A deliberately inheritable fd in the supervisor (dup clears FD_CLOEXEC), as if inherited from a harness.
    let f = std::fs::File::open("/proc/self/stat").unwrap();
    // SAFETY: dup of a valid fd; closed at the end of the test.
    let leaked = unsafe { libc::dup(f.as_raw_fd()) };
    assert!(leaked >= 3);
    assert!(
        rustred_campaign::launch::audit_inheritable_fds()
            .unwrap()
            .iter()
            .any(|(fd, _)| *fd == leaked)
    );
    let list = "for f in /proc/$$/fd/*; do [ -e \"$f\" ] && echo \"${f##*/}\"; done; exit 0";
    let mut l = launcher();
    let mut h = l.launch(&sh(dir.path(), "cloexec", list)).unwrap();
    let r = drive(&mut h);
    assert_eq!(r.exit, Some(ExitState::Exited(0)));
    let out = std::fs::read_to_string(dir.path().join("stdout.log")).unwrap();
    let mut fds: Vec<i32> = out.lines().filter_map(|l| l.trim().parse().ok()).collect();
    fds.sort();
    assert_eq!(fds, vec![0, 1, 2], "child fds: {out}");
    // Explicit inheritance (e.g. a CPU-lease flock) is honoured.
    let dir2 = TempDir::new("cloexec2");
    let mut spec = sh(dir2.path(), "cloexec2", list);
    spec.inherit_fds = vec![leaked];
    let mut h2 = l.launch(&spec).unwrap();
    drive(&mut h2);
    let out2 = std::fs::read_to_string(dir2.path().join("stdout.log")).unwrap();
    assert!(
        out2.lines().any(|l| l.trim() == leaked.to_string()),
        "inherited fd missing: {out2}"
    );
    // SAFETY: closing the fd this test created.
    unsafe { libc::close(leaked) };
}

#[test]
fn leftover_helper_after_normal_exit_is_killed_and_recorded() {
    let _g = serial();
    let dir = TempDir::new("leftover");
    let mut l = launcher();
    let mut h = l
        .launch(&sh(dir.path(), "leftover", "sleep 600 & sleep 1; exit 7"))
        .unwrap();
    let sid = h.identity().pid;
    let r = drive(&mut h);
    assert_eq!(r.exit, Some(ExitState::Exited(7)));
    assert_eq!(r.kill_reason, None);
    assert_eq!(r.leftovers_killed.len(), 1, "{r:?}");
    assert_eq!(r.leftovers_killed[0].comm, "sleep");
    assert!(r.session_verified_empty);
    assert_session_gone(sid);
}

#[test]
fn missing_passthrough_variable_fails_by_name_only() {
    let _g = serial();
    let dir = TempDir::new("env");
    let mut l = launcher();
    let mut spec = sh(dir.path(), "env", "exit 0");
    spec.env_passthrough = vec!["RR_PROC_TEST_SURELY_UNSET_VARIABLE".into()];
    let e = l.launch(&spec).err().expect("must fail");
    assert_eq!(
        e.to_string(),
        "environment variable RR_PROC_TEST_SURELY_UNSET_VARIABLE is not set"
    );
    // Clean environment + explicit allow-list: the child sees exactly what the spec gives.
    let mut spec = sh(dir.path(), "env2", "env | sort");
    spec.env.push(("RR_GIVEN".into(), "1".into()));
    spec.env_passthrough = vec!["HOME".into()];
    let mut h = l.launch(&spec).unwrap();
    drive(&mut h);
    let out = std::fs::read_to_string(dir.path().join("stdout.log")).unwrap();
    let names: Vec<&str> = out
        .lines()
        .filter_map(|l| l.split_once('=').map(|x| x.0))
        .collect();
    for n in &names {
        assert!(
            [
                "PATH",
                "HOME",
                "RR_GIVEN",
                "PWD",
                "SHLVL",
                "_",
                "OLDPWD",
                "RR_CAMPAIGN_JOB"
            ]
            .contains(n),
            "unexpected variable {n} in {names:?}"
        );
    }
    assert!(names.contains(&"RR_GIVEN") && names.contains(&"HOME"));
    // The campaign marker is always set: <campaign id>:<label>.
    assert!(
        out.lines()
            .any(|l| l.starts_with("RR_CAMPAIGN_JOB=adhoc-") && l.ends_with(":env2")),
        "{out}"
    );
}

#[test]
fn reattach_after_restart_kills_a_recorded_child() {
    let _g = serial();
    let dir = TempDir::new("reattach");
    let mut first = launcher();
    let mut h = first
        .launch(&sh(dir.path(), "reattach", "sleep 600 & wait"))
        .unwrap();
    let id = h.identity().clone();
    let s = Arc::clone(first.sampler());
    wait_until("2 members", LONG, || {
        h.poll().unwrap();
        s.latest()
            .and_then(|x| x.session(id.pid).cloned())
            .is_some_and(|u| u.members.len() >= 2)
    });
    // "Restarted supervisor": a fresh launcher that knows only the recorded identity.
    let mut second: RealLauncher = launcher();
    let Reattached::Live(mut adopted) = second
        .reattach(
            &id,
            "reattach",
            ReattachChoice::Kill {
                grace: Duration::from_secs(3),
            },
        )
        .unwrap()
    else {
        panic!("recorded child not found")
    };
    let r = drive(&mut adopted);
    assert_eq!(r.kill_reason, Some(KillReason::Reattach));
    assert_eq!(r.exit, Some(ExitState::Unknown));
    assert!(r.session_verified_empty, "{r:?}");
    // The original parent reaps its zombie.
    let r0 = drive(&mut h);
    assert!(r0.exit.and_then(|e| e.signal()).is_some());
    assert_session_gone(id.pid);
    // Gone now: a further reattach reports it lost.
    assert!(matches!(
        second
            .reattach(&id, "reattach", ReattachChoice::Adopt)
            .unwrap(),
        Reattached::Lost(_)
    ));
}
