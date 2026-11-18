//! Regression tests for the findings of audits A (notes/implementer.audit-proc-A.md) and B
//! (notes/implementer.audit-proc-B.md) of the process layer. One test per finding id (name prefix `a_` / `b_`).
//!
//! RUN ONLY INSIDE A PRIVATE PID NAMESPACE (coordinator rule L10):
//! `LIGHT_PIDNS=1 WS/plan/auto-campaign/implementer_ops/light_build.sh procfix test -p rustred-campaign --test proc_audit_regressions`
//! Inside, this binary is pid 1 of an empty namespace and sees / can signal only its own descendants. Tests that FORCE
//! pid reuse write /proc/sys/kernel/ns_last_pid, which only works there; they refuse to run (and fail loudly) outside.
//!
//! DECOYS: processes of the test's own making stand in for "foreign" jobs (no campaign marker, another campaign's marker,
//! a recycled pid, a leaderless session). Every decoy must SURVIVE what the layer does; each test then removes its own
//! decoys and asserts that nothing of it is left.
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

#[path = "proc_common/mod.rs"]
mod common;

use common::*;
use rustred_campaign::admission::{
    self, AdmissionPolicy, Candidate, Decision, GB, HostView, LowState, RunningJob, WaitReason,
};
use rustred_campaign::cpu_policy::{
    Acquire, CpuPolicy, SITE_SCHEMA, SiteConfig, SitePolicy, ledger_claims_strict, sanitize,
};
use rustred_campaign::launch::{
    CapRequest, ChildHandle, ChildLauncher, ChildProgress, ExitState, KillReason, LaunchSpec,
    LostReason, MemoryMechanism, RealConfig, RealLauncher, ReattachChoice, Reattached, Refusal,
    UsageReading, Verdict, Verification, adhoc_campaign_id, kill_orphan, peek_exit,
    signal_verified_process,
};
use rustred_campaign::resources::{
    CampaignMarker, InvalidPid, MARKER_VAR, ProcessIdentity, ReadOutcome, Sampler, SamplerConfig,
    SamplerHealth, ValidPid, boot_id, parse_cpu_list, read_bounded, read_stat, scan_campaign,
};
use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

// ------------------------------------------------------------------------------------------------ helpers

fn in_namespace() -> bool {
    std::process::id() == 1
}

/// Tests that need the namespace (forced pid reuse) fail loudly outside it instead of silently passing.
fn require_namespace() {
    assert!(
        in_namespace(),
        "this test must run inside a private pid namespace (LIGHT_PIDNS=1 light_build.sh ...)"
    );
}

fn campaign() -> String {
    adhoc_campaign_id()
}

fn marker(label: &str) -> CampaignMarker {
    CampaignMarker::new(campaign(), label).unwrap()
}

fn alive(c: &mut Child) -> bool {
    c.try_wait().unwrap().is_none()
}

/// Live (not zombie) process with this start time?
fn pid_alive(pid: u32, start: u64) -> bool {
    read_stat(pid)
        .ok()
        .is_some_and(|s| s.start_ticks == start && !s.is_dead())
}

fn sh_cmd(script: &str) -> Command {
    let mut c = Command::new("/bin/sh");
    c.arg("-c").arg(script).stdin(Stdio::null());
    c
}

/// A decoy in its OWN session (setsid), optionally carrying an environment marker value.
fn decoy_session(script: &str, marker_value: Option<&str>) -> Child {
    let mut c = sh_cmd(script);
    c.env_remove(MARKER_VAR);
    if let Some(v) = marker_value {
        c.env(MARKER_VAR, v);
    }
    // SAFETY: setsid in the forked child before exec (async-signal-safe).
    unsafe {
        c.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    c.spawn().unwrap()
}

/// Wait for a decoy of ours (a setsid'd child spawned OUTSIDE the launcher). procfix7 round 15 (1 of 100 full runs under
/// load: a_c1_decoy_with_other_campaign_marker_survives, "No child processes"): the launcher's process-wide
/// adopted-children reaper may take such a child's exit status first (its documented contract, resources/children.rs:
/// every zombie child that is not a registered leader and not in our own session is reaped by exact pid); the wait then
/// fails with ECHILD and the decoy is gone just the same. Any other error fails the test.
fn wait_decoy(c: &mut Child) {
    match c.wait() {
        Ok(_) => {}
        Err(e) if e.raw_os_error() == Some(libc::ECHILD) => {}
        Err(e) => panic!("wait for decoy {}: {e}", c.id()),
    }
}

/// Spawn `make()` until the child gets pid `want` (ns_last_pid = want - 1 before each try). Namespace only.
/// procfix7 round 14: bounded by TIME (60 s, a 10-ms pause after a failed try), no longer by 200 back-to-back tries:
/// a task of this namespace that takes `want` between the write and the spawn (a thread of this test process, a process
/// of a job) holds it for its whole life, and 200 tries could all fall inside that life ("could not force pid 45": 2 of
/// 312 full runs under load in round 14, at a422bfa9 and 39043e23; the holder was not caught in 1,300 runs, a failure
/// now names it).
fn force_pid(want: u32, mut make: impl FnMut() -> Child) -> Child {
    require_namespace();
    let t0 = Instant::now();
    loop {
        fs::write("/proc/sys/kernel/ns_last_pid", (want - 1).to_string()).unwrap();
        let mut c = make();
        if c.id() == want {
            return c;
        }
        let holder = fs::read_to_string(format!("/proc/{want}/status"))
            .map(|s| {
                s.lines()
                    .filter(|l| {
                        ["Name:", "State:", "Tgid:", "PPid:"]
                            .iter()
                            .any(|p| l.starts_with(p))
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_else(|e| format!("none ({e})"));
        let _ = c.kill();
        let _ = c.wait();
        assert!(
            t0.elapsed() < Duration::from_secs(60),
            "could not force pid {want} within 60 s (held by: {holder})"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Kill a decoy's orphaned member (our descendant, in our namespace) after checking its start time.
fn kill_decoy_member(pid: u32, start: u64) {
    if pid_alive(pid, start) {
        // SAFETY: a positive pid of our own decoy, verified by start time just now, inside our pid namespace.
        unsafe { libc::kill(pid as i32, libc::SIGKILL) };
    }
    wait_until("decoy member gone", Duration::from_secs(30), || {
        !pid_alive(pid, start)
    });
}

fn read_pid_file(p: &Path) -> u32 {
    wait_until("pid file", Duration::from_secs(30), || {
        fs::read_to_string(p).is_ok_and(|s| s.trim().parse::<u32>().is_ok())
    });
    fs::read_to_string(p).unwrap().trim().parse().unwrap()
}

fn launcher_cfg(f: impl FnOnce(&mut RealConfig)) -> RealLauncher {
    let mut c = RealConfig::default();
    f(&mut c);
    RealLauncher::with_config(sampler(), c)
}

fn record_and_end_a_child(dir: &Path, label: &str) -> ProcessIdentity {
    let mut l = launcher();
    let mut h = l.launch(&sh(dir, label, "exec sleep 600")).unwrap();
    let id = h.identity().clone();
    h.kill_now(KillReason::Other("test".into()));
    let r = drive(&mut h);
    assert!(r.verification.is_verified(), "{r:?}");
    assert_session_gone(id.pid);
    id
}

// ------------------------------------------------------------------------------------------------ helper modes

/// Helper entry for re-executed modes (does nothing in a normal run).
#[test]
fn helper_entry() {
    match std::env::var("RR_PROC_HELPER").as_deref() {
        Ok("alloc") => {
            let mut blocks: Vec<Vec<u8>> = Vec::new();
            for _ in 0..25 {
                let mut b = vec![0u8; 10_000_000];
                for i in (0..b.len()).step_by(4096) {
                    b[i] = 1;
                }
                blocks.push(b);
                std::thread::sleep(Duration::from_millis(50));
            }
            std::thread::sleep(Duration::from_secs(600));
            drop(blocks);
        }
        Ok("cow") => {
            // 80 MB touched, then fork: the child shares every page (copy-on-write, no write).
            let mut b = vec![0u8; 80_000_000];
            for i in (0..b.len()).step_by(4096) {
                b[i] = 1;
            }
            // SAFETY: fork in a test helper; the child only sleeps (no allocation, no locks taken).
            let pid = unsafe { libc::fork() };
            if pid == 0 {
                loop {
                    // SAFETY: sleep is async-signal-safe.
                    unsafe { libc::sleep(600) };
                }
            }
            std::thread::sleep(Duration::from_secs(600));
            drop(b);
        }
        Ok("thread-leader") => {
            // libtest runs this on a worker thread, so fork: in the child the calling thread IS the main thread. The child
            // leads its own session (setsid), starts a thread, and its main thread exits alone (SYS_exit, not
            // exit_group): a zombie main thread with a live thread, carrying the job's marker.
            let out = std::path::PathBuf::from(std::env::var("RR_PROC_OUT").unwrap());
            // SAFETY: fork in a test helper; glibc makes malloc usable in the child.
            let pid = unsafe { libc::fork() };
            if pid == 0 {
                // SAFETY: setsid in the child.
                unsafe { libc::setsid() };
                std::thread::spawn(|| std::thread::sleep(Duration::from_secs(600)));
                let me = std::process::id();
                fs::write(out.join("tl.tmp"), me.to_string()).unwrap();
                fs::rename(out.join("tl.tmp"), out.join("tl.pid")).unwrap();
                std::thread::sleep(Duration::from_millis(200));
                // SAFETY: exit of the calling (main) thread only.
                unsafe { libc::syscall(libc::SYS_exit, 0) };
            }
            std::thread::sleep(Duration::from_secs(600));
        }
        Ok("sigchld-ign") => {
            let out = std::path::PathBuf::from(std::env::var("RR_PROC_OUT").unwrap());
            let was_ignored = rustred_campaign::launch::sigchld_autoreaps();
            let mut l = launcher();
            let setup = l.setup().sigchld;
            let mut h = l.launch(&sh(&out, "sigchld-child", "exit 3")).unwrap();
            let r = drive(&mut h);
            let line = format!(
                "was_ignored={was_ignored} setup={setup:?} autoreaps_now={} exit={:?}",
                rustred_campaign::launch::sigchld_autoreaps(),
                r.exit
            );
            fs::write(out.join("sigchld.txt"), line).unwrap();
        }
        Ok("minisup") => {
            // A supervisor that launches a job with a grandchild (no PDEATHSIG of its own) and records the campaign.
            let out = std::path::PathBuf::from(std::env::var("RR_PROC_OUT").unwrap());
            let mut l = launcher();
            let h = l
                .launch(&sh(
                    &out,
                    "orph",
                    "sleep 600 & echo $! > gc.pid; exec sleep 600",
                ))
                .unwrap();
            fs::write(out.join("campaign.tmp"), l.config().campaign_id.clone()).unwrap();
            fs::rename(out.join("campaign.tmp"), out.join("campaign")).unwrap();
            std::thread::sleep(Duration::from_secs(600));
            drop(h);
        }
        _ => {}
    }
}

fn helper(dir: &Path, label: &str, mode: &str) -> LaunchSpec {
    let mut s = helper_spec(dir, label, "helper_entry", mode);
    s.env.push(("RR_PROC_OUT".into(), dir.into()));
    s
}

// ------------------------------------------------------------------------------------------------ audit A

/// A-C1 (the audit's own reproduction): the recorded leader is gone, its pid is REUSED by a decoy that leads its own
/// session (and in a second round: a decoy session whose leader exited, leaving a member). A restarted supervisor
/// reattaches with Kill: verdict "not ours" (Lost), every decoy survives.
#[test]
fn a_c1_recorded_pid_reused_by_decoy_session_survives() {
    let _g = serial();
    require_namespace();
    let dir = TempDir::new("a-c1");
    // Round 1: decoy session leader with a member, on the recorded pid.
    let id = record_and_end_a_child(dir.path(), "c1");
    let mut d = force_pid(id.pid, || decoy_session("sleep 600 & exec sleep 600", None));
    std::thread::sleep(Duration::from_millis(300));
    let mut second = launcher();
    let r = second
        .reattach(
            &id,
            "c1",
            ReattachChoice::Kill {
                grace: Duration::ZERO,
            },
        )
        .unwrap();
    assert!(
        matches!(r, Reattached::Lost(LostReason::Recycled)),
        "{:?}",
        lost(&r)
    );
    std::thread::sleep(Duration::from_millis(500));
    assert!(alive(&mut d), "decoy leader must survive");
    let sids: BTreeSet<u32> = [id.pid].into();
    let members = rustred_campaign::resources::scan_sessions_now(&sids);
    assert_eq!(members[&id.pid].members.len(), 2, "decoy session intact");
    d.kill().unwrap();
    wait_decoy(&mut d);
    for m in &members[&id.pid].members {
        if m.pid != id.pid {
            kill_decoy_member(m.pid, m.start_ticks);
        }
    }
    // Round 2: LEADERLESS decoy session on the recorded pid (leader exits, member stays).
    let id2 = record_and_end_a_child(dir.path(), "c1b");
    let pidfile = dir.path().join("member.pid");
    let script = format!("sleep 600 & echo $! > {}; exit 0", pidfile.display());
    let mut lead = force_pid(id2.pid, || decoy_session(&script, None));
    wait_decoy(&mut lead);
    let member = read_pid_file(&pidfile);
    let mstart = read_stat(member).unwrap().start_ticks;
    let r = second
        .reattach(
            &id2,
            "c1b",
            ReattachChoice::Kill {
                grace: Duration::ZERO,
            },
        )
        .unwrap();
    assert!(
        matches!(r, Reattached::Lost(LostReason::Gone)),
        "{:?}",
        lost(&r)
    );
    std::thread::sleep(Duration::from_millis(500));
    assert!(
        pid_alive(member, mstart),
        "member of the leaderless decoy session must survive"
    );
    kill_decoy_member(member, mstart);
}

fn lost<H>(r: &Reattached<H>) -> String {
    match r {
        Reattached::Lost(l) => format!("Lost({l:?})"),
        Reattached::Live(_) => "LIVE (adopted)".into(),
    }
}

/// A-C1 (marker): a decoy carrying ANOTHER campaign's marker with the same label, and one with our campaign but another
/// attempt: neither is adopted, signalled by the verified path, or killed by the orphan kill.
#[test]
fn a_c1_decoy_with_other_campaign_marker_survives() {
    let _g = serial();
    let dir = TempDir::new("a-c1m");
    let mut other = decoy_session("exec sleep 600", Some("some-other-campaign:c1m"));
    let mine_other_attempt_value = format!("{}:not-c1m", campaign());
    let mut other_attempt = decoy_session("exec sleep 600", Some(&mine_other_attempt_value));
    std::thread::sleep(Duration::from_millis(300));
    let id = ProcessIdentity::capture(other.id()).unwrap();
    let mut l = launcher();
    for choice in [
        ReattachChoice::Adopt,
        ReattachChoice::Kill {
            grace: Duration::ZERO,
        },
    ] {
        let r = l.reattach(&id, "c1m", choice).unwrap();
        assert!(
            matches!(r, Reattached::Lost(LostReason::NotOurs { .. })),
            "{:?}",
            lost(&r)
        );
    }
    let v = signal_verified_process(&id, &marker("c1m"), libc::SIGKILL);
    assert!(
        matches!(v, Verdict::NotOurs(Refusal::OtherMarker { .. })),
        "{v:?}"
    );
    let id2 = ProcessIdentity::capture(other_attempt.id()).unwrap();
    let v = signal_verified_process(&id2, &marker("c1m"), libc::SIGKILL);
    assert!(
        matches!(v, Verdict::NotOurs(Refusal::OtherMarker { .. })),
        "{v:?}"
    );
    // Orphan scan of OUR campaign lists the other-attempt decoy (it carries our marker) but never the other campaign's.
    let scan = scan_campaign(&campaign(), 0, &BTreeSet::new()).unwrap();
    assert!(!scan.processes.iter().any(|p| p.pid == other.id()));
    // The orphan kill for a third campaign id refuses our-campaign decoy.
    if let Some(p) = scan.processes.iter().find(|p| p.pid == other_attempt.id()) {
        let v = kill_orphan("third-campaign", &boot_id().unwrap(), p, libc::SIGKILL);
        assert!(
            matches!(v, Verdict::NotOurs(Refusal::OtherMarker { .. })),
            "{v:?}"
        );
    }
    std::thread::sleep(Duration::from_millis(300));
    assert!(
        alive(&mut other) && alive(&mut other_attempt),
        "decoys must survive"
    );
    for mut d in [other, other_attempt] {
        d.kill().unwrap();
        wait_decoy(&mut d);
    }
    drop(dir);
}

/// A-M1: a group number from a scan is never signalled as a group. Our job's job-control group ends; a decoy takes the
/// SAME number as a leaderless foreign group; stopping the job must not touch the decoy.
#[test]
fn a_m1_stale_group_number_reused_by_decoy_group_survives() {
    let _g = serial();
    require_namespace();
    let dir = TempDir::new("a-m1");
    let mut l = launcher();
    let gfile = dir.path().join("g.pid");
    let fifo = dir.path().join("g.fifo");
    // procfix7 round 14 (round 13's suite: "timed out waiting for: job-control group seen", 1 failure in 24 suites): the
    // group was `sleep 2`, a group that lived 2 s of WALL CLOCK, and the test needed a scan of the sampler (every 0.2 s)
    // and a poll of its own (every 0.1 s) inside those 2 s; a group that ended unseen is never seen. A host stall that
    // keeps this test process off the CPU for ~2 s breaks that: the failure fell inside the host's user-manager stall of
    // 2026-11-18 05:07-05:14Z (HOST-STEP-3-ANALYSIS 1.1). The product was not involved: 624 full runs of this binary under
    // load (48 in parallel, load 150-199) at a422bfa9, 39043e23 and 041e7fe8 saw the group in 9 error-free scans every
    // time (the stat re-read of procfs.rs and the walk's back-off never cost a scan), and this process held off the CPU
    // for 2.5 s at the group's birth fails the old test at every one of those commits. A TIMING ASSUMPTION of the test,
    // removed: the group now lives until the handle has named it. Its member (`cat` of a FIFO) ends when the test closes
    // the FIFO; the shell reaps it at once (`wait`: its number is free) and becomes the job's `sleep` (no later fork).
    let script = format!(
        "mkfifo {f}; set -m; cat {f} > /dev/null & echo $! > {g}; wait $!; exec sleep 600",
        f = fifo.display(),
        g = gfile.display()
    );
    let mut h = l.launch(&sh(dir.path(), "m1", &script)).unwrap();
    let sid = h.identity().pid;
    let g = read_pid_file(&gfile);
    wait_until("job-control group seen", LONG, || {
        h.poll().unwrap();
        h.usage().is_some_and(|u| u.groups.contains(&g))
    });
    // The handle's last usage names g. The group ends now: a non-blocking open of the FIFO for writing succeeds once its
    // reader (the group's member) has it open (before: ENXIO, tried again); closing it at once gives the reader EOF.
    wait_until("the group's FIFO opened for writing", LONG, || {
        fs::OpenOptions::new()
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&fifo)
            .is_ok()
    });
    wait_until("group ended and reaped", LONG, || {
        read_stat(g).ok().is_none()
    });
    // The handle's last usage still names group g. A decoy now creates a leaderless group g in its own session.
    let mfile = dir.path().join("dm.pid");
    let dscript = format!("sleep 600 & echo $! > {}; exit 0", mfile.display());
    let mut dl = force_pid(g, || decoy_session(&dscript, None));
    wait_decoy(&mut dl);
    let dm = read_pid_file(&mfile);
    let dstart = read_stat(dm).unwrap().start_ticks;
    assert_eq!(read_stat(dm).unwrap().pgrp, g, "decoy member is in group g");
    h.terminate(Duration::from_secs(2), KillReason::Stop);
    let r = drive(&mut h);
    assert!(r.verification.is_verified(), "{r:?}");
    assert_session_gone(sid);
    assert!(
        pid_alive(dm, dstart),
        "the decoy's leaderless group g must survive"
    );
    kill_decoy_member(dm, dstart);
}

/// A-M2: the control side learns that the sampler is stale; usage readings say Stale; a verification that cannot
/// get a fresh scan ends as an explicit Unverified (zombie kept), never silently.
#[test]
fn a_m2_stale_sampler_is_reported_and_verify_is_unverified() {
    let _g = serial();
    let s = Sampler::spawn(SamplerConfig {
        interval: Duration::from_secs(3600),
        collect_foreign_pinning: false,
        foreign_pin_max_cpus: usize::MAX,
    })
    .unwrap();
    s.request_now();
    wait_until("first scan", LONG, || s.latest().is_some());
    std::thread::sleep(Duration::from_millis(300));
    assert!(matches!(
        s.health(Duration::from_millis(100)),
        SamplerHealth::Stale {
            thread_alive: true,
            ..
        }
    ));
    assert!(s.health(Duration::from_secs(3600)).is_fresh());
    // stale_after = 0: no snapshot ever counts as fresh -> usage Stale. Since the second fix package (procfix2) the end of
    // a job is PROVEN by the end-of-job hunt, which does not depend on the sampler at all: a stale sampler no longer makes
    // the verification give up (the explicit Unverified path is exercised by proc_reaudit_regressions rb_c1_*).
    let dir = TempDir::new("a-m2");
    let mut l = launcher_cfg(|c| {
        c.stale_after = Duration::ZERO;
        c.verify_timeout = Duration::from_secs(2);
    });
    let mut h = l.launch(&sh(dir.path(), "m2", "exit 0")).unwrap();
    let pid = h.identity().pid;
    let r = drive(&mut h);
    assert!(matches!(h.usage_reading(), UsageReading::Stale { .. }));
    assert!(
        matches!(r.verification, Verification::Verified { .. }),
        "{:?}",
        r.verification
    );
    assert!(r.session_verified_empty);
    drop(h);
    wait_until("leader reaped", Duration::from_secs(30), || {
        peek_exit(pid).is_err()
    });
}

/// A-M3 (property): with stale or skewed job measurements (arbitrary values, unrelated to the truth), a batch never
/// admits below the floor in the worst case where every job grows to its cap.
#[test]
fn a_m3_stale_or_skewed_measurements_never_admit_below_floor() {
    let mut x = 0x5DEE_CE66_D1CE_4E5Bu64;
    let mut rnd = |n: u64| {
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D) % n.max(1)
    };
    let mut admitted = 0usize;
    for _ in 0..30_000 {
        let p = AdmissionPolicy::new(64);
        let now = Duration::from_secs(1000);
        let free_without_us = (50 + rnd(400)) * GB;
        let mut used_true = 0u64;
        let running: Vec<RunningJob> = (0..rnd(8))
            .map(|i| {
                let cap = (1 + rnd(60)) * GB;
                let truth = rnd(cap / GB + 1) * GB;
                used_true += truth;
                let fresh = rnd(2) == 0;
                // Fresh: truthful and recent. Not fresh: an ARBITRARY value, measured long ago or skewed.
                let (rss, at) = if fresh {
                    (truth, Some(now - Duration::from_secs(rnd(5))))
                } else {
                    let at = match rnd(3) {
                        0 => None,
                        1 => Some(now - Duration::from_secs(11 + rnd(100))),
                        _ => Some(now - Duration::from_secs(11 + rnd(5))),
                    };
                    (rnd(cap / GB + 1) * GB, at)
                };
                RunningJob {
                    id: 100 + i,
                    cap_bytes: cap,
                    expected_peak_bytes: cap,
                    rss_bytes: rss,
                    rss_measured_at: at,
                    cores: 1,
                    started_seq: i,
                    cpu_seconds: 0.0,
                    stopping: false,
                }
            })
            .collect();
        let host = HostView {
            mem_available_bytes: free_without_us.saturating_sub(used_true),
            measured_at: now - Duration::from_secs(rnd(3)),
            now,
        };
        let queue: Vec<Candidate> = (0..rnd(6))
            .map(|i| {
                let peak = rnd(60) * GB;
                Candidate {
                    id: i,
                    expected_peak_bytes: peak,
                    cap_bytes: peak + rnd(5) * GB,
                    cores: 1,
                    bypassed: 0,
                }
            })
            .collect();
        let out = admission::decide_batch(&p, host, &running, &queue);
        let caps: u64 = queue
            .iter()
            .filter(|c| out.admitted().contains(&c.id))
            .map(|c| c.cap_bytes)
            .sum();
        admitted += out.admitted().len();
        // Worst case: every running job grows to its cap, every admitted job to its cap.
        let grow: u64 = running.iter().map(|j| j.cap_bytes).sum::<u64>() - used_true;
        let worst = host.mem_available_bytes as i128 - grow as i128 - caps as i128;
        if !out.admitted().is_empty() {
            assert!(
                worst >= p.floor_bytes as i128,
                "{running:?} {host:?} {queue:?} {out:?}"
            );
        }
    }
    assert!(admitted > 1000, "property exercised: {admitted}");
    // A stale host reading admits nothing.
    let p = AdmissionPolicy::new(8);
    let host = HostView {
        mem_available_bytes: 1000 * GB,
        measured_at: Duration::ZERO,
        now: Duration::from_secs(60),
    };
    let c = Candidate {
        id: 1,
        expected_peak_bytes: GB,
        cap_bytes: GB,
        cores: 1,
        bypassed: 0,
    };
    assert!(matches!(
        admission::admit_one(&p, host, &[], &c),
        Decision::Wait(WaitReason::StaleMeasurements { .. })
    ));
}

/// A-M4: a launcher started with SIGCHLD inherited as SIG_IGN resets it, and its child's exit is observed normally.
#[test]
fn a_m4_sigchld_ignored_at_start_is_reset() {
    let _g = serial();
    let dir = TempDir::new("a-m4");
    let exe = std::env::current_exe().unwrap();
    let mut c = Command::new(exe);
    c.args(["--exact", "helper_entry", "--nocapture", "--test-threads=1"])
        .env("RR_PROC_HELPER", "sigchld-ign")
        .env("RR_PROC_OUT", dir.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null());
    // SAFETY: signal() in the forked child before exec; SIG_IGN for SIGCHLD survives exec.
    unsafe {
        c.pre_exec(|| {
            libc::signal(libc::SIGCHLD, libc::SIG_IGN);
            Ok(())
        });
    }
    let st = c.status().unwrap();
    assert!(st.success());
    let out = fs::read_to_string(dir.path().join("sigchld.txt")).unwrap();
    assert!(out.contains("was_ignored=true"), "{out}");
    assert!(out.contains("setup=ResetFromIgnored"), "{out}");
    assert!(out.contains("autoreaps_now=false"), "{out}");
    assert!(out.contains("exit=Some(Exited(3))"), "{out}");
}

/// A-M5: bounded reads come back within their bound on a read that blocks forever (a FIFO without writer), and a handle
/// dropped while its child runs does not block (the reap goes to a helper thread).
#[test]
fn a_m5_no_blocking_in_the_control_path() {
    let _g = serial();
    let dir = TempDir::new("a-m5");
    let fifo = dir.path().join("fifo");
    let cpath = std::ffi::CString::new(fifo.to_str().unwrap()).unwrap();
    // SAFETY: mkfifo with a valid C string.
    assert_eq!(unsafe { libc::mkfifo(cpath.as_ptr(), 0o600) }, 0);
    let t0 = Instant::now();
    assert!(matches!(
        read_bounded(&fifo, Duration::from_millis(200)),
        ReadOutcome::TimedOut
    ));
    assert!(t0.elapsed() < Duration::from_secs(3));
    let _w = fs::OpenOptions::new().write(true).open(&fifo).unwrap(); // releases the stuck reader
    // Drop of a running handle: quick, and the job dies.
    let mut l = launcher();
    let h = l
        .launch(&sh(dir.path(), "m5", "trap '' TERM; sleep 600"))
        .unwrap();
    let sid = h.identity().pid;
    let t0 = Instant::now();
    drop(h);
    assert!(
        t0.elapsed() < Duration::from_secs(2),
        "Drop blocked for {:?}",
        t0.elapsed()
    );
    assert_session_gone(sid);
}

fn site_raw(root: &Path, cpus: &[usize], tag: &str) -> SiteConfig {
    fs::create_dir_all(root.join("locks")).unwrap();
    SiteConfig {
        schema: SITE_SCHEMA.into(),
        tag: tag.into(),
        lock_dir: root.join("locks"),
        lock_name: "cpu.{n}".into(),
        ledger_file: root.join("CPU_LEDGER.md"),
        ledger_scan_lines: 1500,
        candidates: Some(
            cpus.iter()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(","),
        ),
        exclude: vec![],
        held_files: vec![],
        pause_files: vec![],
        idle_min: 0.0,
        max_cores: 16,
        respect_foreign_pinning: false,
        foreign_pin_max_cpus: 64,
        ignore_own_tag_claims: true,
    }
}

fn cpus4() -> Vec<usize> {
    rustred_campaign::resources::allowed_cpus()
        .unwrap()
        .into_iter()
        .filter(|c| !(168..=175).contains(c) && !(288..=319).contains(c))
        .take(4)
        .collect()
}

/// A-M6: a child inherits the lease's flock (the CPU stays locked after the lease is released while the job lives);
/// at restart, own claims without RELEASED whose flock is free get a "reconciled" RELEASED line; held ones stay.
#[test]
fn a_m6_lease_fd_inherited_and_restart_reconciliation() {
    let _g = serial();
    let dir = TempDir::new("a-m6");
    let c = cpus4();
    let cfg = site(dir.path(), &c, "rr-procfix-test");
    let mut p = SitePolicy::new(cfg.clone()).unwrap();
    let lease = p.acquire(1, "m6 job").unwrap().granted().unwrap();
    let cpu = lease.cpus()[0];
    let mut l = launcher();
    let spec = sh(dir.path(), "m6", "exec sleep 600").with_lease(&lease);
    let mut h = l.launch(&spec).unwrap();
    lease.release().unwrap();
    let lockf = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(dir.path().join(format!("locks/cpu.{cpu}")))
        .unwrap();
    assert!(
        lockf.try_lock().is_err(),
        "the job still holds the CPU's flock"
    );
    h.kill_now(KillReason::Other("test".into()));
    drive(&mut h);
    wait_until("flock free after the job", Duration::from_secs(30), || {
        lockf.try_lock().is_ok()
    });
    lockf.unlock().unwrap();
    // Restart reconciliation: two stale own claims, one with a held flock.
    let free_cpu = c[1];
    let held_cpu = c[2];
    let held = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.path().join(format!("locks/cpu.{held_cpu}")))
        .unwrap();
    held.try_lock().unwrap();
    let mut text = fs::read_to_string(&cfg.ledger_file).unwrap_or_default();
    text.push_str(&format!(
        "2026-11-14T00:00:00Z (rr-procfix-test): + CPU {free_cpu} (flock) -> crashed job. Released at its end.\n\
         2026-11-14T00:00:01Z (rr-procfix-test): + CPU {held_cpu} (flock) -> orphan job. Released at its end.\n"
    ));
    fs::write(&cfg.ledger_file, text).unwrap();
    let mut q = SitePolicy::new(cfg.clone()).unwrap();
    let rec = q.reconcile_own_claims().unwrap();
    assert_eq!(rec.released, vec![free_cpu]);
    assert_eq!(rec.held, vec![held_cpu]);
    let text = fs::read_to_string(&cfg.ledger_file).unwrap();
    assert!(
        text.lines()
            .last()
            .unwrap()
            .contains("reconciled at restart"),
        "{text}"
    );
    let v = ledger_claims_strict(text.lines());
    let own = v.claims.get("rr-procfix-test").cloned().unwrap_or_default();
    assert!(!own.contains(&free_cpu) && own.contains(&held_cpu));
    held.unlock().unwrap();
}

/// A-M7: the launcher detects and reports the cap mechanisms; inside a private namespace the systemd scope is refused
/// (it would see foreign pids); the mechanism used is recorded per job.
#[test]
fn a_m7_cap_mechanism_detected_and_recorded() {
    let _g = serial();
    let dir = TempDir::new("a-m7");
    let mut l = launcher();
    if rustred_campaign::launch::in_private_namespace() {
        assert!(l.setup().systemd_run.is_none());
        assert!(
            l.setup()
                .systemd_unavailable
                .as_deref()
                .unwrap()
                .contains("namespace")
        );
    }
    let mut spec = sh(dir.path(), "m7", "exit 0");
    spec.memory_cap_bytes = Some(1 << 30);
    let r = drive(&mut l.launch(&spec).unwrap());
    let expect = if l.setup().systemd_run.is_some() {
        matches!(r.memory_mechanism, MemoryMechanism::SystemdScope { .. })
    } else {
        r.memory_mechanism == MemoryMechanism::Sampling
    };
    assert!(
        expect,
        "{:?} with setup {:?}",
        r.memory_mechanism,
        l.setup()
    );
    let mut req = spec.clone();
    req.cap_mechanism = CapRequest::SystemdScope;
    if l.setup().systemd_run.is_none() {
        assert!(
            l.launch(&req).is_err(),
            "a required scope that is unavailable fails the launch"
        );
    }
}

/// Subreaper: a double-forked descendant is re-parented to the supervisor (not to init), carries the marker, is killed
/// at the job's end and reaped (no zombie left).
#[test]
fn a_subreaper_double_fork_is_found_killed_and_reaped() {
    let _g = serial();
    let dir = TempDir::new("a-sub");
    let mut l = launcher();
    assert!(rustred_campaign::launch::is_child_subreaper());
    let f = dir.path().join("d.pid");
    let script = format!(
        "( setsid sh -c 'echo $$ > {}; exec sleep 600' & ); sleep 1; exit 0",
        f.display()
    );
    let mut h = l.launch(&sh(dir.path(), "sub", &script)).unwrap();
    let d = read_pid_file(&f);
    let st = read_stat(d).unwrap();
    assert_eq!(st.ppid, std::process::id(), "re-parented to the subreaper");
    let r = drive(&mut h);
    assert!(r.verification.is_verified(), "{r:?}");
    assert!(r.leftovers_killed.iter().any(|x| x.pid == d), "{r:?}");
    wait_until("daemon reaped (no zombie)", Duration::from_secs(60), || {
        read_stat(d)
            .ok()
            .is_none_or(|s| s.start_ticks != st.start_ticks)
    });
}

/// A-m1 / B-C1 / B-C2: pid 0, 1, negative, our own pid, above pid_max in a record: a typed error before any syscall,
/// nothing signalled. (Tested at the validation level only: the layer must never be able to send such a signal.)
#[test]
fn a_minor_m1_and_b_c1_c2_invalid_pids_are_typed_errors() {
    let b = boot_id().unwrap();
    for js in [
        "-1",
        "0",
        "1",
        "4194305",
        "2147483648",
        "4294967295",
        "4294967296",
        "1.5",
    ] {
        let t = format!("{{\"pid\":{js},\"start_ticks\":1,\"boot_id\":\"{b}\"}}");
        assert!(
            serde_json::from_str::<ProcessIdentity>(&t).is_err(),
            "pid {js} accepted"
        );
    }
    for p in [-1i64, 0, 1, 1 << 31, i64::from(u32::MAX)] {
        assert!(
            ValidPid::new(p).is_err() && ValidPid::for_signal(p).is_err(),
            "{p}"
        );
    }
    let me = i64::from(std::process::id());
    assert!(ValidPid::for_signal(me).is_err());
    // SAFETY: getppid / getsid cannot fail.
    let (pp, sid) = unsafe { (libc::getppid(), libc::getsid(0)) };
    for p in [pp, sid] {
        if p > 1 {
            assert!(ValidPid::for_signal(i64::from(p)).is_err());
        }
    }
    assert!(ProcessIdentity::capture(1).is_err() && ProcessIdentity::capture(0).is_err());
    let mut l = launcher();
    for pid in [0u32, 1, u32::MAX, 4_194_305, std::process::id()] {
        let id = ProcessIdentity {
            pid,
            start_ticks: 1,
            boot_id: b.clone(),
        };
        let e = l
            .reattach(
                &id,
                "x",
                ReattachChoice::Kill {
                    grace: Duration::ZERO,
                },
            )
            .err()
            .unwrap_or_else(|| panic!("pid {pid} accepted by reattach"));
        assert!(
            e.get_ref()
                .and_then(|x| x.downcast_ref::<InvalidPid>())
                .is_some(),
            "{e}"
        );
        assert!(matches!(
            signal_verified_process(&id, &marker("x"), libc::SIGKILL),
            Verdict::NotOurs(Refusal::InvalidPid(_))
        ));
        assert!(id.check().is_err());
    }
}

/// A-m2: a ZOMBIE that is not our child is never signalled (it reads as Gone).
#[test]
fn a_minor_m2_zombie_non_child_is_not_signalled() {
    let _g = serial();
    let dir = TempDir::new("a-m2z");
    let mut l = launcher();
    let f = dir.path().join("z.pid");
    // The background sh exits; its parent execs `sleep`, which never reaps it: a zombie that is not our child.
    let script = format!("sh -c 'echo $$ > {}; exit 0' & exec sleep 600", f.display());
    let mut h = l.launch(&sh(dir.path(), "m2z", &script)).unwrap();
    let z = read_pid_file(&f);
    wait_until("zombie", Duration::from_secs(30), || {
        read_stat(z).ok().is_none_or(|s| s.is_dead())
    });
    if let Ok(st) = read_stat(z) {
        let id = ProcessIdentity {
            pid: z,
            start_ticks: st.start_ticks,
            boot_id: boot_id().unwrap(),
        };
        assert_eq!(
            signal_verified_process(&id, &marker("m2z"), libc::SIGKILL),
            Verdict::Gone
        );
    }
    h.kill_now(KillReason::Other("test".into()));
    drive(&mut h);
}

/// A-m4 (fail closed): unparsable claim lines count as claimed.
#[test]
fn a_minor_m4_unparsable_claims_fail_closed() {
    let lines = [
        "2026-11-13 09:20 (x): + CPU 5 for something",
        "no owner here + CPU 9 (flock)",
        "2026-11-13T23:00:37Z (y): + CPU 7 (flock) -> ok.",
    ];
    let v = ledger_claims_strict(lines);
    // pick_free's owner rule needs "<token> (owner)": "2026-11-13 09:20 (x)" has none -> unknown owner, still claimed.
    assert!(v.claims.values().any(|s| s.contains(&5)));
    assert!(v.claims.values().any(|s| s.contains(&9)));
    assert!(v.claims["y"].contains(&7));
    assert_eq!(v.unparsable_lines, 2);
}

/// A-m5: labels cannot inject claim / release tokens or newlines; a line is one write of at most PIPE_BUF bytes.
#[test]
fn a_minor_m5_labels_sanitised_and_lines_atomic() {
    let s = sanitize("evil) RELEASED 3,4 (x\n+ CPU 9 ; done");
    assert!(
        !s.contains("RELEASED") && !s.contains('+') && !s.contains('\n') && !s.contains(')'),
        "{s}"
    );
    let dir = TempDir::new("a-m5l");
    let p = dir.path().join("L");
    // lane int5 (gate3 m3): the site ledger is never created: a missing one is refused and stays missing
    assert!(rustred_campaign::cpu_policy::append_line(&p, "ok").is_err());
    assert!(!p.exists());
    fs::write(&p, b"").unwrap();
    assert!(rustred_campaign::cpu_policy::append_line(&p, &"x".repeat(5000)).is_err());
    assert!(rustred_campaign::cpu_policy::append_line(&p, "a\nb").is_err());
    rustred_campaign::cpu_policy::append_line(&p, "ok").unwrap();
    assert_eq!(fs::read_to_string(&p).unwrap(), "ok\n");
}

/// A-m6 / B-m6: the ledger window is the true last N lines: one 800 KB line does not hide an earlier foreign claim.
#[test]
fn a_minor_m6_b_minor_m6_huge_line_does_not_hide_claims() {
    let _g = serial();
    let dir = TempDir::new("a-m6t");
    let c = cpus4();
    let cfg = site(dir.path(), &c, "rr-procfix-test");
    let mut text = String::new();
    for cpu in &c[..3] {
        text.push_str(&format!(
            "2026-11-14T00:00:00Z (foreign): + CPU {cpu} (flock) -> long job.\n"
        ));
    }
    text.push_str(&"z".repeat(800_000));
    text.push('\n');
    fs::write(&cfg.ledger_file, text).unwrap();
    let mut p = SitePolicy::new(cfg).unwrap();
    let lease = p
        .acquire(1, "t")
        .unwrap()
        .granted()
        .expect("the one free CPU");
    assert_eq!(lease.cpus(), &[c[3]]);
    assert!(matches!(p.acquire(1, "t").unwrap(), Acquire::NotNow(_)));
}

/// A-m8: the cgroup headroom gets its own margin (0), not the host floor.
#[test]
fn a_minor_m8_cgroup_headroom_has_its_own_margin() {
    let m = rustred_campaign::resources::MemInfo {
        mem_total: 500 * GB,
        mem_available: 400 * GB,
        cgroup_headroom: Some(10 * GB),
    };
    assert_eq!(m.effective_available_for(50 * GB), 60 * GB);
    assert!(
        admission::choose_victims(
            &AdmissionPolicy::new(8),
            HostView::at(m.effective_available_for(50 * GB)),
            &[],
            LowState {
                samples: 9,
                since: Duration::ZERO
            }
        )
        .is_none()
    );
}

/// A-m10: SIGHUP never escalates (a closing pane's HUP + TERM is ONE request); Ctrl-Z (SIGTSTP) does not stop us.
#[test]
fn a_minor_m10_hangup_does_not_escalate_and_tstp_is_ignored() {
    let _g = serial();
    let s = rustred_campaign::signals::StopSignals::install().unwrap();
    let h = s.handle();
    // SAFETY: signalling ourselves with signals whose handlers are installed.
    unsafe { libc::raise(libc::SIGHUP) };
    unsafe { libc::raise(libc::SIGTERM) };
    wait_until("hangup", Duration::from_secs(10), || h.hangup_seen());
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(h.level(), rustred_campaign::signals::StopLevel::Drain);
    unsafe { libc::raise(libc::SIGTSTP) };
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(
        h.level(),
        rustred_campaign::signals::StopLevel::Drain,
        "still running"
    );
    drop(s);
}

// ------------------------------------------------------------------------------------------------ audit B

/// B-C3 (port of I2e): leaderless decoy session + a record of an OLDER process with that pid: refused, member alive.
#[test]
fn b_c3_leaderless_session_with_older_record_is_refused() {
    let _g = serial();
    let dir = TempDir::new("b-c3");
    let f = dir.path().join("m.pid");
    let mut lead = decoy_session(
        &format!("sleep 600 & echo $! > {}; sleep 0.3; exit 0", f.display()),
        None,
    );
    let lead_pid = lead.id();
    let lead_start = read_stat(lead_pid).unwrap().start_ticks;
    wait_decoy(&mut lead);
    let m = read_pid_file(&f);
    let mstart = read_stat(m).unwrap().start_ticks;
    let rec = ProcessIdentity {
        pid: lead_pid,
        start_ticks: lead_start.saturating_sub(100),
        boot_id: boot_id().unwrap(),
    };
    let mut l = launcher();
    let r = l
        .reattach(
            &rec,
            "tamper",
            ReattachChoice::Kill {
                grace: Duration::from_secs(1),
            },
        )
        .unwrap();
    assert!(matches!(r, Reattached::Lost(_)), "{}", lost(&r));
    std::thread::sleep(Duration::from_millis(500));
    assert!(pid_alive(m, mstart), "decoy member must survive");
    kill_decoy_member(m, mstart);
}

/// B-C4: (a) a double-forked daemon is found by its marker, killed and REPORTED (T2); (c) a setsid escapee's memory
/// counts against the cap; (d) a grandchild that outlives a SIGKILLed supervisor is found by the orphan scan of the
/// restarted one and killed through the verified orphan kill.
#[test]
fn b_c4_orphans_daemons_and_escapees() {
    let _g = serial();
    let dir = TempDir::new("b-c4");
    let mut l = launcher();
    // (a) T2. procfix7 round 16 (gate17 nit m17-bc4: 5 of 48 runs under load, 8 of 48 at 041e7fe8 "timed out waiting
    // for: pid file"): the job exited at once, and the end-of-job proof could kill its daemon before the daemon wrote its
    // pid (correctly: a leftover); the job now waits (bounded, 10 s) until the daemon's pid file is there (written
    // atomically) before it exits: the daemon is the same leftover, found by its marker, killed and reported
    let f = dir.path().join("d.pid");
    let script = format!(
        "( setsid sh -c 'echo $$ > {f}.tmp && mv {f}.tmp {f}; exec sleep 20' & ); \
         i=0; while [ ! -s {f} ] && [ $i -lt 200 ]; do sleep 0.05; i=$((i+1)); done; exit 0",
        f = f.display()
    );
    let mut h = l.launch(&sh(dir.path(), "t2", &script)).unwrap();
    let r = drive(&mut h);
    let d = read_pid_file(&f);
    assert!(
        r.leftovers_killed.iter().any(|x| x.pid == d),
        "daemon reported: {r:?}"
    );
    assert!(r.verification.is_verified(), "{r:?}");
    wait_until("daemon dead", Duration::from_secs(30), || {
        read_stat(d)
            .ok()
            .is_none_or(|s| s.is_dead() || s.comm != "sleep")
    });
    // (c) setsid escapee allocating beyond the cap.
    let exe = std::env::current_exe().unwrap();
    let mut spec = sh(
        dir.path(),
        "esc-mem",
        &format!(
            "setsid '{}' --exact helper_entry --nocapture --test-threads=1 & wait",
            exe.display()
        ),
    );
    spec.env.push(("RR_PROC_HELPER".into(), "alloc".into()));
    spec.memory_cap_bytes = Some(80_000_000);
    spec.cap_mechanism = CapRequest::Sampling;
    let mut h = l.launch(&spec).unwrap();
    let r = drive(&mut h);
    assert!(
        matches!(r.kill_reason, Some(KillReason::MemoryCap { .. })),
        "{r:?}"
    );
    assert!(r.verification.is_verified(), "{r:?}");
    // (d) orphan after a supervisor SIGKILL.
    let mut sup = l.launch(&helper(dir.path(), "minisup", "minisup")).unwrap();
    let cfile = dir.path().join("campaign");
    wait_until("helper supervisor ready", LONG, || cfile.exists());
    let gc = read_pid_file(&dir.path().join("gc.pid"));
    let camp = fs::read_to_string(&cfile).unwrap();
    sup.kill_now(KillReason::Other("test kills the helper supervisor".into()));
    drive(&mut sup);
    let gstat = read_stat(gc).expect("grandchild survives its supervisor");
    let scan = scan_campaign(&camp, 0, &BTreeSet::new()).unwrap();
    let p = scan
        .processes
        .iter()
        .find(|p| p.pid == gc)
        .expect("found by the orphan scan");
    assert_eq!(p.attempt_id, "orph");
    assert_eq!(
        kill_orphan(&camp, &boot_id().unwrap(), p, libc::SIGKILL),
        Verdict::Delivered
    );
    wait_until("orphan gone", Duration::from_secs(30), || {
        !pid_alive(gc, gstat.start_ticks)
    });
}

/// B-M1: an adopted job whose leader's main thread exited while a thread lives is NOT reported finished.
#[test]
fn b_m1_adopted_zombie_leader_with_live_thread_keeps_running() {
    let _g = serial();
    let dir = TempDir::new("b-m1");
    let mut first = launcher();
    let mut h = first
        .launch(&helper(dir.path(), "tl", "thread-leader"))
        .unwrap();
    let tl = read_pid_file(&dir.path().join("tl.pid"));
    let id = ProcessIdentity::capture(tl).unwrap();
    wait_until("main thread exited", LONG, || {
        read_stat(id.pid).ok().is_some_and(|s| s.is_dead())
    });
    let mut second = launcher();
    let Reattached::Live(mut a) = second.reattach(&id, "tl", ReattachChoice::Adopt).unwrap() else {
        panic!("not adopted")
    };
    for _ in 0..20 {
        assert!(
            !matches!(a.poll().unwrap(), ChildProgress::Finished(_)),
            "reported finished while a thread runs"
        );
        std::thread::sleep(Duration::from_millis(150));
    }
    drop(a);
    // The launching handle's job end kills the marked thread-leader (outside its session) through the checklist.
    h.kill_now(KillReason::Other("test".into()));
    let r = drive(&mut h);
    assert!(r.verification.is_verified(), "{r:?}");
    let tstart = id.start_ticks;
    wait_until(
        "thread leader gone (no live thread)",
        Duration::from_secs(30),
        || {
            read_stat(tl).ok().is_none_or(|s| s.start_ticks != tstart)
                || !matches!(
                    rustred_campaign::resources::live_thread(tl),
                    Ok(rustred_campaign::resources::LiveThread::Found(..))
                )
        },
    );
}

/// B-M2 (port of I2d): the CORRECT identity of a decoy that is not a session leader (and has no marker): refused.
#[test]
fn b_m2_correct_identity_of_non_session_leader_is_refused() {
    let _g = serial();
    let mut c = sh_cmd("exec sleep 600");
    c.process_group(0).env_remove(MARKER_VAR);
    let mut d = c.spawn().unwrap();
    std::thread::sleep(Duration::from_millis(200));
    let id = ProcessIdentity::capture(d.id()).unwrap();
    let mut l = launcher();
    for choice in [
        ReattachChoice::Adopt,
        ReattachChoice::Kill {
            grace: Duration::from_secs(1),
        },
    ] {
        let r = l.reattach(&id, "tamper", choice).unwrap();
        assert!(
            matches!(r, Reattached::Lost(LostReason::NotOurs { .. })),
            "{}",
            lost(&r)
        );
    }
    std::thread::sleep(Duration::from_millis(500));
    assert!(alive(&mut d));
    d.kill().unwrap();
    d.wait().unwrap();
}

/// B-M3: CPU ranges are bounded: "0-60000000" is an error at once (site exclude, held file, --cpus), never an abort.
#[test]
fn b_m3_cpu_ranges_are_bounded() {
    let t0 = Instant::now();
    assert!(parse_cpu_list("0-60000000").is_err());
    assert!(parse_cpu_list("4000000000").is_err());
    assert!(rustred_campaign::cpu_policy::CpuSpec::parse("0-60000000").is_err());
    assert!(t0.elapsed() < Duration::from_secs(1));
    let dir = TempDir::new("b-m3");
    let mut cfg = site(dir.path(), &cpus4(), "rr-procfix-test");
    cfg.exclude = vec!["0-60000000".into()];
    assert!(SitePolicy::new(cfg.clone()).is_err());
    cfg.exclude = vec![];
    fs::write(dir.path().join("held.json"), r#"{"held":"0-60000000"}"#).unwrap();
    cfg.held_files = vec![rustred_campaign::cpu_policy::HeldFile {
        path: dir.path().join("held.json"),
        key: "held".into(),
        required: false,
    }];
    let mut p = SitePolicy::new(cfg).unwrap();
    assert!(
        matches!(p.acquire(1, "x").unwrap(), Acquire::NotNow(_)),
        "fail closed"
    );
}

fn job(id: u64, rss: u64, seq: u64, stopping: bool) -> RunningJob {
    RunningJob {
        id,
        cap_bytes: rss * GB + GB,
        expected_peak_bytes: rss * GB,
        rss_bytes: rss * GB,
        rss_measured_at: Some(Duration::from_secs(10)),
        cores: 1,
        started_seq: seq,
        cpu_seconds: 0.0,
        stopping,
    }
}

/// B-M4 (ports of A5a-c): minimal victim sets; whole_campaign iff every holder is a victim; victims in flight count.
#[test]
fn b_m4_emergency_victims_minimal_and_in_flight() {
    let p = AdmissionPolicy::new(64);
    let host = |gb: u64| HostView {
        mem_available_bytes: gb * GB,
        measured_at: Duration::from_secs(10),
        now: Duration::from_secs(10),
    };
    let low = LowState {
        samples: 3,
        since: Duration::ZERO,
    };
    // A5a: the oldest alone covers deficit 20 + 2.
    let jobs = [
        job(1, 30, 0, false),
        job(2, 5, 1, false),
        job(3, 5, 2, false),
    ];
    let plan = admission::choose_victims(&p, host(30), &jobs, low).unwrap();
    assert_eq!(plan.victims, vec![1]);
    assert!(!plan.whole_campaign);
    // A5b: redundant victim dropped (2 alone suffices for 8 + 2).
    let jobs = [
        job(1, 40, 0, false),
        job(2, 12, 1, false),
        job(3, 3, 2, false),
    ];
    let plan = admission::choose_victims(&p, host(42), &jobs, low).unwrap();
    assert_eq!(plan.victims, vec![2]);
    // All holders needed: whole campaign (30 + 12 + 3 < 52).
    let jobs = [
        job(1, 30, 0, false),
        job(2, 12, 1, false),
        job(3, 3, 2, false),
    ];
    let plan = admission::choose_victims(&p, host(0), &jobs, low).unwrap();
    assert!(plan.whole_campaign && plan.victims.len() == 3);
    // A5c: victims being killed stay in `running` with stopping = true: nothing more is chosen while they cover it.
    let jobs = [
        job(1, 40, 0, false),
        job(2, 12, 1, true),
        job(3, 3, 2, false),
    ];
    let plan = admission::choose_victims(&p, host(42), &jobs, low).unwrap();
    assert!(plan.victims.is_empty(), "{plan:?}");
}

/// B-m2: copy-on-write sharing (80 MB + fork) is not double counted: PSS keeps the job under a 120 MB cap.
#[test]
fn b_minor_m2_cow_pages_not_double_counted() {
    let _g = serial();
    let dir = TempDir::new("b-m2c");
    let mut l = launcher();
    let mut spec = helper(dir.path(), "cow", "cow");
    spec.memory_cap_bytes = Some(120_000_000);
    spec.cap_mechanism = CapRequest::Sampling;
    let mut h = l.launch(&spec).unwrap();
    let t0 = Instant::now();
    let mut seen_two = false;
    while t0.elapsed() < Duration::from_secs(8) {
        assert!(
            !matches!(h.poll().unwrap(), ChildProgress::Finished(_)),
            "killed: CoW counted twice"
        );
        seen_two |= h
            .usage()
            .is_some_and(|u| u.members.len() >= 2 && u.pss_bytes.is_some());
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(seen_two, "the fork was sampled with PSS");
    let u = h.usage().unwrap();
    assert!(
        u.rss_bytes > 120_000_000,
        "RSS double counts ({})",
        u.rss_bytes
    );
    h.terminate(Duration::from_secs(2), KillReason::Stop);
    let r = drive(&mut h);
    assert_eq!(r.kill_reason, Some(KillReason::Stop));
}

/// B-m3: terminate() after the leader exited on its own does not mark the job interrupted.
#[test]
fn b_minor_m3_terminate_after_exit_is_not_interrupted() {
    let _g = serial();
    let dir = TempDir::new("b-m3t");
    let mut l = launcher();
    let mut h = l.launch(&sh(dir.path(), "t12", "exit 0")).unwrap();
    wait_until("exit seen", LONG, || h.poll_exit().unwrap().is_some());
    h.terminate(Duration::from_secs(5), KillReason::Stop);
    let r = drive(&mut h);
    assert_eq!(r.exit, Some(ExitState::Exited(0)));
    assert_eq!(r.kill_reason, None);
    assert!(!r.sigterm_sent);
}

/// B-m4: an adopted job whose leader is gone gets SIGTERM first (grace), not only SIGKILL.
#[test]
fn b_minor_m4_adopted_leaderless_job_gets_sigterm_first() {
    let _g = serial();
    let dir = TempDir::new("b-m4a");
    let mut first = launcher();
    let mark = dir.path().join("got-term");
    let script = format!(
        "sh -c 'trap \"touch {}; exit 0\" TERM; sleep 600 & wait' & sleep 0.5; exit 0",
        mark.display()
    );
    let mut h = first.launch(&sh(dir.path(), "s3c", &script)).unwrap();
    let id = h.identity().clone();
    drive_until_exit(&mut h);
    let mut second = launcher();
    let Reattached::Live(mut a) = second
        .reattach(
            &id,
            "s3c",
            ReattachChoice::Kill {
                grace: Duration::from_secs(3),
            },
        )
        .unwrap()
    else {
        panic!("leaderless job not found by its marker")
    };
    let r = drive(&mut a);
    assert!(r.verification.is_verified(), "{r:?}");
    assert!(mark.exists(), "the member got SIGTERM before any SIGKILL");
    drop(h);
}

fn drive_until_exit<H: ChildHandle>(h: &mut H) {
    wait_until("leader exit", LONG, || h.poll_exit().unwrap().is_some());
}

/// B-m7: absurd inputs do not admit through saturating arithmetic.
#[test]
fn b_minor_m7_checked_admission_arithmetic() {
    let p = AdmissionPolicy::new(8);
    let c = Candidate {
        id: 1,
        expected_peak_bytes: 1,
        cap_bytes: u64::MAX - 10,
        cores: 1,
        bypassed: 0,
    };
    assert!(matches!(
        admission::admit_one(&p, HostView::at(u64::MAX), &[], &c),
        Decision::Wait(_)
    ));
    let mut q = p.clone();
    q.campaign_mem_cap_bytes = Some(u64::MAX);
    let r = [RunningJob {
        id: 9,
        cap_bytes: u64::MAX - 5,
        expected_peak_bytes: 1,
        rss_bytes: 0,
        rss_measured_at: Some(Duration::ZERO),
        cores: 1,
        started_seq: 0,
        cpu_seconds: 0.0,
        stopping: false,
    }];
    let c2 = Candidate {
        id: 2,
        expected_peak_bytes: 15,
        cap_bytes: 15,
        cores: 1,
        bypassed: 0,
    };
    assert!(matches!(
        admission::admit_one(&q, HostView::at(u64::MAX), &r, &c2),
        Decision::Wait(_)
    ));
}

/// B-m8: a passthrough "NAME=value" misuse is refused without echoing the value (error and Debug).
#[test]
fn b_minor_m8_passthrough_misuse_never_echoes_the_value() {
    let mut s = LaunchSpec::new("x", "/bin/true", Path::new("/tmp"));
    s.env_passthrough
        .push("SYMBOLICA_LICENSE=AUDIT-DUMMY-VALUE".into());
    let e = s.validate().unwrap_err().to_string();
    assert!(!e.contains("AUDIT-DUMMY-VALUE"), "{e}");
    assert!(!format!("{s:?}").contains("AUDIT-DUMMY-VALUE"));
    let mut l = launcher();
    let e = l.launch(&s).err().unwrap().to_string();
    assert!(!e.contains("AUDIT-DUMMY-VALUE"));
}

// ------------------------------------------------------------------------------------------------ static check

/// The crate's sources contain NO group / broadcast signal outside the one wrapper (launch/guard.rs, module `sys`),
/// and every other `libc::kill(` is listed with its reason.
#[test]
fn no_signal_syscall_outside_the_verified_wrapper() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    let mut stack = vec![src.clone()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                files.push(p);
            }
        }
    }
    let forbidden = [
        "kill(-",
        "killpg(",
        "kill(0",
        "SYS_kill",
        "tgkill",
        "tkill(",
        "pidfd_send_signal",
    ];
    let mut report = Vec::new();
    for f in &files {
        let rel = f.strip_prefix(&src).unwrap().display().to_string();
        let text = fs::read_to_string(f).unwrap();
        for (i, line) in text.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            for pat in forbidden.iter().chain(["libc::kill("].iter()) {
                if code.contains(pat) {
                    report.push((rel.clone(), i + 1, pat.to_string(), line.trim().to_string()));
                }
            }
        }
    }
    for (f, n, pat, line) in &report {
        println!("GREP {f}:{n}: [{pat}] {line}");
    }
    // In guard.rs: exactly one killpg and one libc::kill, both inside `mod sys`, plus the pidfd_send_signal syscall.
    let outside: Vec<_> = report
        .iter()
        .filter(|(f, _, pat, _)| {
            if f == "launch/guard.rs" {
                return false;
            }
            // The contract lane's fake child (a test double): kill of its own registered helpers and of itself, with a
            // positive pid; not part of the process layer and never a group or broadcast signal.
            !(f == "bin/rr_fake_child.rs" && pat == "libc::kill(")
        })
        .collect();
    assert!(
        outside.is_empty(),
        "signal syscalls outside the wrapper: {outside:?}"
    );
    let guard: Vec<_> = report
        .iter()
        .filter(|(f, ..)| f == "launch/guard.rs")
        .collect();
    assert_eq!(
        guard.iter().filter(|x| x.2 == "killpg(").count(),
        1,
        "{guard:?}"
    );
    assert_eq!(
        guard.iter().filter(|x| x.2 == "libc::kill(").count(),
        1,
        "{guard:?}"
    );
    assert!(
        !report
            .iter()
            .any(|(_, _, pat, line)| pat == "kill(-"
                || (pat == "kill(0" && !line.contains("kill(0x")))
    );
}

/// gate3 m3 (lane int5): the campaign never creates the site's files: a test site has its ledger and one lock file per
/// CPU (0..=1023 would be too many: the candidate CPUs and the excluded ones) before `SitePolicy::new`.
fn make_site_files(cfg: &SiteConfig, cpus: &[usize]) {
    std::fs::create_dir_all(&cfg.lock_dir).unwrap();
    if !cfg.ledger_file.exists() {
        std::fs::write(&cfg.ledger_file, b"").unwrap();
    }
    for c in cpus {
        let p = cfg
            .lock_dir
            .join(cfg.lock_name.replace("{n}", &c.to_string()));
        if !p.exists() {
            std::fs::write(&p, b"").unwrap();
        }
    }
}

fn site(root: &Path, cpus: &[usize], tag: &str) -> SiteConfig {
    let c = site_raw(root, cpus, tag);
    make_site_files(&c, cpus);
    c
}
