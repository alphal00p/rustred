//! Regression tests for the findings of the two GATE RE-AUDITS of the process layer (notes/implementer.reaudit-proc-A.md,
//! notes/implementer.reaudit-proc-B.md). One test per finding id (prefix `rb_` = re-audit B, `ra_` = re-audit A).
//!
//! RUN ONLY INSIDE A PRIVATE PID NAMESPACE (coordinator rule L10):
//! `LIGHT_PIDNS=1 WS/plan/auto-campaign/implementer_ops/light_build.sh procfix2 test -p rustred-campaign --test proc_reaudit_regressions`
//! Inside, this binary is pid 1 of an empty namespace: orphans are re-parented to it (as to the supervisor's subreaper),
//! and it can see and signal only its own descendants.
//!
//! "Fails before": each test names the behaviour of the code before the second fix package (70947c6b + supervisor lane)
//! that it catches; the port of re-audit B's harness (tests/proc_reaudit_b.rs) reproduces the same on the old code.

#[path = "proc_common/mod.rs"]
mod common;

use common::*;
use rustred_campaign::admission::{AdmissionPolicy, HostView, MeasurementError, RunningJob};
use rustred_campaign::cpu_policy::{Acquire, CpuPolicy, SITE_SCHEMA, SiteConfig, SitePolicy};
use rustred_campaign::launch::{
    CapPreference, ChildHandle, ChildLauncher, ChildProgress, FinalReport, KillReason, LaunchSpec,
    LauncherError, PidfdProbe, RealChild, RealConfig, RealLauncher, RetireEvent,
    TrackingPreference, Verdict, Verification, kill_orphan, peek_exit, pidfd_probe,
};
use rustred_campaign::resources::{
    CampaignMarker, MARKER_VAR, MarkedProcess, boot_id, read_stat, reaper_stats, scan_campaign,
};
use std::collections::BTreeSet;
use std::fs;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

// ------------------------------------------------------------------------------------------------ helpers

fn require_namespace() {
    assert_eq!(
        std::process::id(),
        1,
        "this test must run inside a private pid namespace (LIGHT_PIDNS=1 light_build.sh ...)"
    );
}

fn cfg(f: impl FnOnce(&mut RealConfig)) -> RealConfig {
    let mut c = RealConfig {
        cap_preference: CapPreference::NoSystemd,
        tracking: TrackingPreference::ProcOnly,
        verify_timeout: Duration::from_secs(30),
        rehunt_every: Duration::from_millis(300),
        ..RealConfig::default()
    };
    f(&mut c);
    c
}

fn launcher_with(f: impl FnOnce(&mut RealConfig)) -> RealLauncher {
    RealLauncher::with_config(sampler(), cfg(f))
}

/// Live (not dead) process with this start time? (A dead main thread with a live thread counts as alive.)
fn alive(pid: u32, start: u64) -> bool {
    read_stat(pid).ok().is_some_and(|s| {
        s.start_ticks == start
            && (!s.is_dead()
                || matches!(
                    rustred_campaign::resources::live_thread(pid),
                    Ok(rustred_campaign::resources::LiveThread::Found(..))
                ))
    })
}

fn read_pid_file(p: &Path) -> u32 {
    wait_until("pid file", Duration::from_secs(60), || {
        fs::read_to_string(p).is_ok_and(|s| s.trim().parse::<u32>().is_ok())
    });
    fs::read_to_string(p).unwrap().trim().parse().unwrap()
}

fn start_of(pid: u32) -> u64 {
    read_stat(pid).unwrap().start_ticks
}

/// SIGKILL a process of the test's own making (our descendant in our namespace) after checking its start time.
fn kill_own(pid: u32, start: u64) {
    if alive(pid, start) {
        // SAFETY: a positive pid of our own descendant, start time verified just now, inside our private namespace.
        unsafe { libc::kill(pid as i32, libc::SIGKILL) };
    }
    wait_until("own process gone", Duration::from_secs(30), || {
        !alive(pid, start)
    });
}

/// Zombie children of this process.
fn my_zombies() -> Vec<u32> {
    let me = std::process::id();
    let mut out = Vec::new();
    for e in fs::read_dir("/proc").unwrap().flatten() {
        if let Some(pid) = e.file_name().to_str().and_then(|s| s.parse::<u32>().ok())
            && let Ok(s) = read_stat(pid)
            && s.ppid == me
            && s.state == 'Z'
        {
            out.push(pid);
        }
    }
    out
}

/// Absolute path of a program on PATH.
fn which(name: &str) -> String {
    std::env::split_paths(&std::env::var_os("PATH").unwrap())
        .map(|d| d.join(name))
        .find(|p| p.is_file())
        .unwrap_or_else(|| panic!("{name} not on PATH"))
        .display()
        .to_string()
}

fn nofile() -> (u64, u64) {
    // SAFETY: getrlimit with a valid out-pointer.
    let mut r: libc::rlimit = unsafe { std::mem::zeroed() };
    unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut r) };
    (r.rlim_cur, r.rlim_max)
}

fn set_nofile(cur: u64, max: u64) {
    let r = libc::rlimit {
        rlim_cur: cur,
        rlim_max: max,
    };
    // SAFETY: setrlimit with a valid pointer (soft <= hard).
    unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &r) };
}

/// Exhaust the descriptor table (soft limit = fds in use + 8, then open /dev/null until EMFILE). Restored on drop.
struct FdStorm {
    held: Vec<fs::File>,
    saved: (u64, u64),
}

impl FdStorm {
    fn start() -> Self {
        let saved = nofile();
        let n = fs::read_dir("/proc/self/fd")
            .map(|d| d.count())
            .unwrap_or(64) as u64;
        set_nofile(n + 8, saved.1);
        let mut held = Vec::new();
        while let Ok(f) = fs::File::open("/dev/null") {
            held.push(f);
            if held.len() > 10_000 {
                break;
            }
        }
        Self { held, saved }
    }
}

impl Drop for FdStorm {
    fn drop(&mut self) {
        self.held.clear();
        set_nofile(self.saved.0, self.saved.1);
    }
}

fn poll_until_finished(h: &mut RealChild, t: Duration) -> Option<FinalReport> {
    let t0 = Instant::now();
    while t0.elapsed() < t {
        if let Ok(ChildProgress::Finished(r)) = h.poll() {
            return Some(r);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    None
}

fn site_cfg_raw(root: &Path, cpu: usize) -> SiteConfig {
    fs::create_dir_all(root.join("locks")).unwrap();
    SiteConfig {
        schema: SITE_SCHEMA.into(),
        tag: "rr-procfix2-test".into(),
        lock_dir: root.join("locks"),
        lock_name: "cpu.{n}".into(),
        ledger_file: root.join("CPU_LEDGER.md"),
        ledger_scan_lines: 1500,
        candidates: Some(cpu.to_string()),
        exclude: vec![],
        held_files: vec![],
        pause_files: vec![],
        idle_min: 0.0,
        max_cores: 1,
        respect_foreign_pinning: false,
        ignore_own_tag_claims: true,
    }
}

/// Drive the quarantine until the label is cleared (or panic after `t`).
fn until_cleared(l: &mut RealLauncher, label: &str, t: Duration) -> RetireEvent {
    let t0 = Instant::now();
    loop {
        for ev in l.tick() {
            if let RetireEvent::Cleared { label: x, .. } = &ev
                && x == label
            {
                return ev;
            }
        }
        assert!(
            t0.elapsed() < t,
            "{label} not cleared within {t:?}: {:?}",
            l.quarantined()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

// ------------------------------------------------------------------------------------------------ helper modes

/// Helper entry for re-executed modes (does nothing in a normal run).
#[test]
fn helper_entry() {
    match std::env::var("RR_PROC_HELPER").as_deref() {
        Ok("relay") => relay_helper(),
        Ok("idlock") => idlock_helper(),
        _ => {}
    }
}

/// A marked process that keeps moving: fork, the parent exits, the child calls setsid and repeats every `pause` us.
/// libtest runs this on a worker thread: fork FIRST; the single-threaded child runs the loop with async-signal-safe
/// calls only (fork, setsid, pwrite of the hop count as 8 raw bytes, usleep, _exit); the helper returns at once, so the
/// job's leader exits immediately (re-audit B's `rb helper relay`).
fn relay_helper() {
    let pause: u32 = std::env::var("RR_RELAY_PAUSE_US")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(500);
    let counter = std::env::var("RR_RELAY_COUNTER").unwrap();
    let c = std::ffi::CString::new(counter).unwrap();
    // SAFETY: open with a valid C string.
    let fd = unsafe { libc::open(c.as_ptr(), libc::O_WRONLY | libc::O_CREAT, 0o600) };
    assert!(fd >= 0);
    // SAFETY: fork; the child uses only async-signal-safe calls until it exits.
    let pid = unsafe { libc::fork() };
    if pid == 0 {
        let mut i: u64 = 0;
        loop {
            // SAFETY: as above (single-threaded child).
            unsafe {
                if libc::fork() != 0 {
                    libc::_exit(0);
                }
                libc::setsid();
                i += 1;
                let b = i.to_le_bytes();
                libc::pwrite(fd, b.as_ptr().cast(), 8, 0);
                libc::usleep(pause);
            }
        }
    }
}

fn relay_count(p: &Path) -> u64 {
    fs::read(p)
        .ok()
        .filter(|b| b.len() >= 8)
        .map(|b| u64::from_le_bytes(b[..8].try_into().unwrap()))
        .unwrap_or(0)
}

/// Another PROCESS asking for a campaign id (RB-M2): writes what `try_with_config` and `with_config` said.
fn idlock_helper() {
    let out = PathBuf::from(std::env::var("RR_PROC_OUT").unwrap());
    let id = std::env::var("RR_ID").unwrap();
    let dir = PathBuf::from(std::env::var("RR_LOCKDIR").unwrap());
    let c = RealConfig {
        campaign_id: id.clone(),
        id_lock_dir: Some(dir.clone()),
        cap_preference: CapPreference::NoSystemd,
        ..RealConfig::default()
    };
    let t = match RealLauncher::try_with_config(sampler(), c.clone()) {
        Ok(_) => "accepted".to_string(),
        Err(LauncherError::IdLock(e)) => format!("refused: {e}"),
        Err(e) => format!("error: {e}"),
    };
    let w = RealLauncher::with_config(sampler(), c);
    let line = format!(
        "{t}\nwith_config id={} replaced={:?}\n",
        w.config().campaign_id,
        w.setup().campaign_id_replaced
    );
    fs::write(out.join("idlock.tmp"), line).unwrap();
    fs::rename(out.join("idlock.tmp"), out.join("idlock.txt")).unwrap();
}

// ------------------------------------------------------------------------------------------------ RB-C1

/// RB-C1 (a, b): the leader exits while the supervisor cannot open files (EMFILE). Before: the one-pass VERIFY counted
/// failed reads as "no process" and reported `Verified` while the session member and the marked setsid escapee lived
/// (re-audit B scenario fd3). After: never `Verified` while either lives; once descriptors are back the job is proven
/// gone (both killed).
#[test]
fn rb_c1_fd_exhaustion_never_verified_while_a_member_lives() {
    let _g = serial();
    require_namespace();
    let dir = TempDir::new("rb-c1");
    let d = dir.path().display().to_string();
    let mut l = launcher_with(|c| c.verify_timeout = Duration::from_secs(3));
    let script = format!(
        "sleep 600 & echo $! > {d}/m; setsid sh -c 'echo $$ > {d}/e; exec sleep 600' & sleep 2; exit 0"
    );
    let mut h = l.launch(&sh(dir.path(), "rbc1", &script)).unwrap();
    let m = read_pid_file(&dir.path().join("m"));
    let e = read_pid_file(&dir.path().join("e"));
    let (sm, se) = (start_of(m), start_of(e));
    let _ = h.poll();
    let fin = {
        let _storm = FdStorm::start();
        let r = poll_until_finished(&mut h, Duration::from_secs(10));
        let (am, ae) = (alive(m, sm), alive(e, se));
        if let Some(r) = &r {
            assert!(
                !(r.verification.is_verified() && (am || ae)),
                "Verified while member alive={am} escapee alive={ae}: {r:?}"
            );
        }
        r
    };
    // Descriptors restored: the end is proven (the handle keeps hunting after an Unverified report).
    let t0 = Instant::now();
    let r = loop {
        match h.poll().unwrap() {
            ChildProgress::Finished(r) if r.verification.is_verified() => break r,
            _ => {}
        }
        assert!(t0.elapsed() < LONG, "never proven; first report {fin:?}");
        std::thread::sleep(Duration::from_millis(50));
    };
    assert!(
        !alive(m, sm) && !alive(e, se),
        "member and escapee are dead: {r:?}"
    );
    println!(
        "RB-C1 fd3: report under EMFILE = {:?}; final = {:?}",
        fin.map(|r| r.verification),
        r.verification
    );
}

/// RB-C1 (d): an Unverified end is kept in the launcher's quarantine WITH ITS CPU LEASE (the survivor may still use the
/// CPU); the lease is released only when a later hunt proves the job gone. Before: the supervisor released the lease and
/// dropped the handle on any finished report.
#[test]
fn rb_c1d_unverified_end_keeps_its_lease_until_proven() {
    let _g = serial();
    require_namespace();
    let dir = TempDir::new("rb-c1d");
    let d = dir.path().display().to_string();
    let mut p = SitePolicy::new(site_cfg(&dir.path().join("site"), usable_cpu())).unwrap();
    let lease = p.acquire(1, "rbc1d").unwrap().granted().unwrap();
    let mut l = launcher_with(|c| c.verify_timeout = Duration::from_secs(2));
    let script = format!("setsid sh -c 'echo $$ > {d}/e; exec sleep 600' & sleep 1; exit 0");
    let mut h = l.launch(&sh(dir.path(), "rbc1d", &script)).unwrap();
    let e = read_pid_file(&dir.path().join("e"));
    let se = start_of(e);
    let r = {
        let _storm = FdStorm::start();
        poll_until_finished(&mut h, Duration::from_secs(20)).expect("a report")
    };
    assert!(
        matches!(r.verification, Verification::Unverified { .. }),
        "{r:?}"
    );
    assert!(l.retire(h, Some(lease)).is_none(), "kept in the quarantine");
    assert!(
        matches!(p.acquire(1, "other").unwrap(), Acquire::NotNow(_)),
        "the CPU stays leased while the end is unproven"
    );
    let ev = until_cleared(&mut l, "rbc1d", LONG);
    let RetireEvent::Cleared { report, .. } = ev else {
        unreachable!()
    };
    assert!(report.verification.is_verified(), "{report:?}");
    assert!(
        !alive(e, se),
        "the escapee was killed by the quarantine hunt"
    );
    assert!(
        matches!(p.acquire(1, "other").unwrap(), Acquire::Granted(_)),
        "lease released once proven"
    );
}

// ------------------------------------------------------------------------------------------------ RB-C2

/// RB-C2: a job process that keeps re-forking into new sessions (the relay; leader exits at once). Before: `Verified`
/// at once while the relay ran, and 30 s of scan_campaign + kill_orphan made 0 kills. After, at every fork rate: the
/// job ends killed and Verified, or Unverified and then killed by the quarantine's hunts within 60 s; NEVER Verified
/// while it runs.
#[test]
fn rb_c2_reforking_relay_never_verified_while_it_runs() {
    let _g = serial();
    require_namespace();
    let dir = TempDir::new("rb-c2");
    let mut l = launcher_with(|c| c.verify_timeout = Duration::from_secs(10));
    for (i, pause_us) in [20_000u32, 5_000, 2_000, 500, 100, 0]
        .into_iter()
        .enumerate()
    {
        let label = format!("relay{i}");
        let cf = dir.path().join(format!("{label}.count"));
        let mut s = helper_spec(dir.path(), &label, "helper_entry", "relay");
        s.env
            .push(("RR_RELAY_PAUSE_US".into(), pause_us.to_string().into()));
        s.env.push(("RR_RELAY_COUNTER".into(), cf.clone().into()));
        let t0 = Instant::now();
        let mut h = l.launch(&s).unwrap();
        let r = poll_until_finished(&mut h, Duration::from_secs(60)).expect("a report");
        let c1 = relay_count(&cf);
        std::thread::sleep(Duration::from_secs(1));
        let c2 = relay_count(&cf);
        let running = c2 > c1;
        assert!(
            !(r.verification.is_verified() && running),
            "relay {pause_us} us: Verified while it runs ({c1} -> {c2} hops): {r:?}"
        );
        let how = if r.verification.is_verified() {
            "Verified".to_string()
        } else {
            assert!(l.retire(h, None).is_none());
            let ev = until_cleared(&mut l, &label, Duration::from_secs(60));
            let RetireEvent::Cleared { after, .. } = ev else {
                unreachable!()
            };
            format!("Unverified, then cleared by the quarantine after {after:?}")
        };
        // Stopped for good.
        let a = relay_count(&cf);
        std::thread::sleep(Duration::from_millis(1500));
        assert_eq!(a, relay_count(&cf), "relay {pause_us} us still running");
        println!(
            "RB-C2 relay hop pause {pause_us} us: {how}; hops {a}; total {:?}",
            t0.elapsed()
        );
    }
    wait_until("no zombie left", Duration::from_secs(30), || {
        my_zombies().is_empty()
    });
}

// ------------------------------------------------------------------------------------------------ RB-M1

/// RB-M1 / RA m-4: re-parented children that are not registered leaders are reaped (marked or not); a REGISTERED
/// leader is never reaped by the adopted-children reaper (its zombie pins the session until the launcher releases it).
/// Before: 400 of 500 short-lived double forks stayed zombies.
#[test]
fn rb_m1_adopted_zombies_are_reaped_and_a_registered_leader_never() {
    let _g = serial();
    require_namespace();
    let dir = TempDir::new("rb-m1");
    let mut l = launcher_with(|_| {});
    let before = reaper_stats().reaped_total;
    let (envp, setsid, shp) = (which("env"), which("setsid"), which("sh"));
    let script = format!(
        "for i in $(seq 300); do ( setsid sh -c 'exit 0' & ); done; \
         for i in $(seq 100); do ( {envp} -i {setsid} {shp} -c 'exit 0' & ); done; \
         for i in $(seq 100); do ( sh -c 'exit 0' & ); done; sleep 1; exit 0"
    );
    let mut h = l.launch(&sh(dir.path(), "rbm1", &script)).unwrap();
    let r = drive(&mut h);
    assert!(r.verification.is_verified(), "{r:?}");
    wait_until(
        "every adopted zombie reaped",
        Duration::from_secs(30),
        || my_zombies().is_empty(),
    );
    let after = reaper_stats();
    assert!(after.reaped_total >= before + 400, "{after:?}");
    // The invariant: a registered leader that exited is not reaped by the reaper.
    let mut h = l.launch(&sh(dir.path(), "rbm1-leader", "exit 7")).unwrap();
    let pid = h.identity().pid;
    let ticks = reaper_stats().ticks;
    wait_until("reaper ticks", Duration::from_secs(30), || {
        rustred_campaign::resources::request_reap();
        reaper_stats().ticks >= ticks + 10
    });
    assert!(
        matches!(peek_exit(pid), Ok(Some(_))),
        "registered leader still an unreaped zombie after 10 reaper ticks"
    );
    let r = drive(&mut h);
    assert!(r.verification.is_verified(), "{r:?}");
    wait_until(
        "leader reaped after release",
        Duration::from_secs(30),
        || peek_exit(pid).is_err(),
    );
}

// ------------------------------------------------------------------------------------------------ RB-M2

/// RB-M2: a second PROCESS with the same campaign id is refused (and signals nothing); launchers of ONE process share
/// the lock; the lock goes with the holder. Before: both started, and B's verification and orphan scan killed A's jobs.
#[test]
fn rb_m2_a_second_process_with_the_same_id_is_refused_and_signals_nothing() {
    let _g = serial();
    require_namespace();
    let dir = TempDir::new("rb-m2");
    let locks = dir.path().join("locks");
    let id = format!("rbm2-{}", std::process::id() + start_of_self());
    let c = RealConfig {
        campaign_id: id.clone(),
        id_lock_dir: Some(locks.clone()),
        cap_preference: CapPreference::NoSystemd,
        ..RealConfig::default()
    };
    let mut a = RealLauncher::try_with_config(sampler(), c.clone()).expect("first holder");
    assert!(
        RealLauncher::try_with_config(sampler(), c.clone()).is_ok(),
        "a second launcher of the SAME process shares the lock"
    );
    let mut j = a.launch(&sh(dir.path(), "J1", "exec sleep 600")).unwrap();
    let jid = j.identity().clone();
    let run_helper = |name: &str| {
        let out = dir.path().join(name);
        fs::create_dir_all(&out).unwrap();
        let mut cmd = Command::new(std::env::current_exe().unwrap());
        cmd.args(["--exact", "helper_entry", "--nocapture", "--test-threads=1"])
            .env("RR_PROC_HELPER", "idlock")
            .env("RR_PROC_OUT", &out)
            .env("RR_ID", &id)
            .env("RR_LOCKDIR", &locks)
            .env_remove(MARKER_VAR)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let st = cmd.status();
        // The adopted-children reaper may have taken the helper's status (contract of resources::children): tolerated.
        if let Ok(s) = st {
            assert!(s.success(), "helper failed");
        }
        fs::read_to_string(out.join("idlock.txt")).unwrap()
    };
    let t = run_helper("h1");
    assert!(t.starts_with("refused:") && t.contains("in use"), "{t}");
    assert!(
        t.contains("replaced=Some"),
        "with_config replaces the id: {t}"
    );
    assert!(
        matches!(j.poll().unwrap(), ChildProgress::Running),
        "A's job untouched"
    );
    assert!(jid.is_same_process());
    j.kill_now(KillReason::Other("test".into()));
    assert!(drive(&mut j).verification.is_verified());
    drop(a);
    let t = run_helper("h2");
    assert!(
        t.starts_with("accepted"),
        "lock released with its holder: {t}"
    );
}

fn start_of_self() -> u32 {
    (start_of(std::process::id()) % 100_000) as u32
}

// ------------------------------------------------------------------------------------------------ minors

/// RB m3 / RA m-2: a transient EMFILE never selects a weaker signal path: the probe is never cached as "unsupported",
/// and a verified kill under EMFILE is Unverified (nothing sent). Before: `pidfd_supported()` cached false for the
/// process lifetime and kill(pid) was used afterwards.
#[test]
fn rb_minor_m3_transient_pidfd_failure_is_never_a_fallback() {
    let _g = serial();
    require_namespace();
    let mut d = Command::new("/bin/sh");
    d.args(["-c", "exec sleep 600"])
        .env(MARKER_VAR, "rbm3camp:x")
        .stdin(Stdio::null());
    // SAFETY: setsid between fork and exec.
    unsafe {
        d.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    let mut d: Child = d.spawn().unwrap();
    std::thread::sleep(Duration::from_millis(300));
    let p = MarkedProcess {
        pid: d.id(),
        start_ticks: start_of(d.id()),
        attempt_id: "x".into(),
        ppid: 1,
        pgrp: d.id(),
        session: d.id(),
        comm: "sleep".into(),
        state: 'S',
        rss_bytes: 0,
        cpu_ticks: 0,
    };
    let boot = boot_id().unwrap();
    let (probe, v) = {
        let _storm = FdStorm::start();
        (
            pidfd_probe(),
            kill_orphan("rbm3camp", &boot, &p, libc::SIGKILL),
        )
    };
    assert_ne!(probe, PidfdProbe::Unsupported);
    assert!(matches!(v, Verdict::Unverified(_)), "{v:?}");
    assert!(
        d.try_wait().map(|s| s.is_none()).unwrap_or(false),
        "not signalled"
    );
    assert_eq!(
        pidfd_probe(),
        PidfdProbe::Supported,
        "not cached as unsupported"
    );
    assert_eq!(
        kill_orphan("rbm3camp", &boot, &p, libc::SIGKILL),
        Verdict::Delivered
    );
    let _ = d.wait();
}

/// RB m1 / m2: an unmarked escapee (env -i + setsid) is reported in the job's report (never signalled), and a CPU whose
/// inherited flock it keeps is reported "held by ... pid N" when the lease is released. Before: reported nowhere,
/// ledger said RELEASED, reconcile said held=[].
#[test]
fn rb_minor_m1_m2_unmarked_survivor_reported_with_the_lease_it_holds() {
    let _g = serial();
    require_namespace();
    let dir = TempDir::new("rb-m12");
    let d = dir.path().display().to_string();
    let site = site_cfg(&dir.path().join("site"), usable_cpu());
    let mut p = SitePolicy::new(site.clone()).unwrap();
    let lease = p.acquire(1, "rbm12").unwrap().granted().unwrap();
    let mut l = launcher_with(|_| {});
    // Absolute paths: `env -i` drops PATH (NixOS has no /bin/sleep).
    let (envp, setsid, shp, sleep) = (which("env"), which("setsid"), which("sh"), which("sleep"));
    let script = format!(
        "( {envp} -i {setsid} {shp} -c 'echo $$ > {d}/u; exec {sleep} 600' & ); sleep 1; exit 0"
    );
    let mut h = l
        .launch(&sh(dir.path(), "rbm12", &script).with_lease(&lease))
        .unwrap();
    let u = read_pid_file(&dir.path().join("u"));
    let su = start_of(u);
    let r = drive(&mut h);
    assert!(r.verification.is_verified(), "{r:?}");
    assert!(alive(u, su), "an unmarked escapee is never signalled");
    assert!(
        r.unattributed_children.iter().any(|x| x.pid == u),
        "reported: {r:?}"
    );
    let ev = l.retire(h, Some(lease)).expect("verified: cleared at once");
    let RetireEvent::Cleared { lease_note, .. } = ev else {
        unreachable!()
    };
    let note = lease_note.unwrap_or_default();
    assert!(note.contains(&format!("pid {u}")), "{note}");
    let mut q = SitePolicy::new(site).unwrap();
    let rec = q.reconcile_own_claims().unwrap();
    assert!(rec.held_by.iter().any(|(_, h)| h.contains(&u)), "{rec:?}");
    kill_own(u, su);
}

/// RB m4: an env KEY misused as "NAME=value" is refused without echo and hidden in Debug. Before: Debug printed it.
#[test]
fn rb_minor_m4_env_key_misuse_never_echoed() {
    const SECRET: &str = "RB-M4-DUMMY-SECRET";
    let mut s = LaunchSpec::new("x", "/bin/true", Path::new("/tmp"));
    s.env.push((format!("A={SECRET}").into(), "v".into()));
    s.env.push(("B C".into(), SECRET.into()));
    let e = s.validate().unwrap_err().to_string();
    assert!(!e.contains(SECRET), "{e}");
    let dbg = format!("{s:?}");
    assert!(!dbg.contains(SECRET), "{dbg}");
    let mut l = launcher_with(|_| {});
    let le = l.launch(&s).err().unwrap().to_string();
    assert!(!le.contains(SECRET), "{le}");
}

/// RB m5: a measurement dated in the future or newer than the host reading is a typed input error (treated as stale:
/// the full cap is charged). Before: it counted as fresh (1,145 below-floor admissions in re-audit B's property run).
#[test]
fn rb_minor_m5_future_and_newer_than_host_measurements_are_stale() {
    let p = AdmissionPolicy::new(8);
    let host = HostView {
        mem_available_bytes: 100,
        measured_at: Duration::from_secs(10),
        now: Duration::from_secs(12),
    };
    let job = |t: u64| RunningJob {
        id: 1,
        cap_bytes: 10,
        expected_peak_bytes: 5,
        rss_bytes: 8,
        rss_measured_at: Some(Duration::from_secs(t)),
        cores: 1,
        started_seq: 0,
        cpu_seconds: 0.0,
        stopping: false,
    };
    assert_eq!(
        job(13).measurement_check(&p, &host),
        Err(MeasurementError::InTheFuture)
    );
    assert_eq!(
        job(11).measurement_check(&p, &host),
        Err(MeasurementError::NewerThanHost)
    );
    assert_eq!(job(9).measurement_check(&p, &host), Ok(()));
    assert_eq!(rustred_campaign::admission::charge(&p, &host, &job(11)), 10);
    assert_eq!(rustred_campaign::admission::charge(&p, &host, &job(9)), 2);
}

/// RA m-5: the tripwire now covers std `Child::kill`, shell-outs to kill tools, sigqueue / pthread_kill / rt_*,
/// `cgroup.kill`, and every source directory of the crate. It is shown to catch planted snippets (before: the old pattern
/// list missed every one of them), then run on the real sources.
#[test]
fn ra_m5_tripwire_covers_every_signal_form_and_directory() {
    // procfix3: serialised with the descriptor-exhaustion tests of this binary (it reads every source file; under the
    // default parallel test threads it hit EMFILE while rb_c1 held the table full).
    let _g = serial();
    let planted = [
        ("supervisor/x.rs", "use libc::kill as k; k(pid, 9);"),
        ("cli/x.rs", "child.kill().unwrap();"),
        ("adapters/x.rs", "Command::new(\"pkill\").arg(\"-f\")"),
        ("bin/x.rs", "Command::new(\"kill\").arg(\"-9\")"),
        ("supervisor/y.rs", "libc::sigqueue(pid, 9, v)"),
        ("supervisor/z.rs", "libc::pthread_kill(t, 9)"),
        (
            "supervisor/w.rs",
            "libc::syscall(libc::SYS_rt_sigqueueinfo, p, 9, i)",
        ),
        (
            "launch/real.rs",
            "fs::write(dir.join(\"cgroup.kill\"), \"1\")",
        ),
        (
            "supervisor/v.rs",
            "Command::new(\"systemctl\").args([\"kill\"])",
        ),
        ("resources/x.rs", "Command::new(\"timeout\").arg(\"-k\")"),
    ];
    for (f, text) in planted {
        assert!(
            !tripwire(f, text).is_empty(),
            "planted {text:?} in {f} not caught"
        );
    }
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
    let dirs: BTreeSet<String> = files
        .iter()
        .filter_map(|f| {
            f.strip_prefix(&src)
                .ok()?
                .components()
                .next()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
        })
        .collect();
    for want in [
        "supervisor",
        "cli",
        "adapters",
        "bin",
        "launch",
        "resources",
    ] {
        assert!(dirs.contains(want), "{want} scanned: {dirs:?}");
    }
    let mut hits = Vec::new();
    for f in &files {
        let rel = f.strip_prefix(&src).unwrap().display().to_string();
        hits.extend(tripwire(&rel, &fs::read_to_string(f).unwrap()));
    }
    for h in &hits {
        println!("TRIPWIRE {h}");
    }
    assert!(hits.is_empty(), "signal paths outside the guard: {hits:?}");
}

/// Every signal-capable form in non-test code of `file`, except the allowed ones (with their reason).
fn tripwire(file: &str, text: &str) -> Vec<String> {
    let forbidden = [
        "kill(",
        "killpg",
        "tgkill",
        "tkill",
        "SYS_kill",
        "pidfd_send_signal",
        "sigqueue",
        "pthread_kill",
        "rt_sigqueueinfo",
        "rt_tgsigqueueinfo",
        ".kill()",
        "cgroup.kill",
        "Command::new(\"kill\"",
        "Command::new(\"pkill\"",
        "Command::new(\"killall\"",
        "Command::new(\"systemctl\"",
        "Command::new(\"timeout\"",
        "Command::new(\"fuser\"",
        "use libc::kill",
        "libc::kill as",
    ];
    let mut out = Vec::new();
    let mut in_sys = false;
    for (i, line) in text.lines().enumerate() {
        // Unit-test modules (at the end of a file by convention) act on the test's own children inside the namespace.
        if line.trim_start().starts_with("#[cfg(test)]") {
            break;
        }
        if file == "launch/guard.rs" {
            if line.starts_with("mod sys {") {
                in_sys = true;
            } else if in_sys && line.starts_with('}') {
                in_sys = false;
            }
        }
        let code = line.split("//").next().unwrap_or("");
        for pat in forbidden {
            if !code.contains(pat) {
                continue;
            }
            // "kill(" also matches the guard's verified API names (kill_orphan(, kill_all(, ...): only a bare call
            // (`kill(` not preceded by an identifier character or `_`) counts.
            if pat == "kill(" {
                let bare = code.match_indices("kill(").any(|(k, _)| {
                    k == 0
                        || !code[..k]
                            .chars()
                            .last()
                            .is_some_and(|c| c.is_alphanumeric() || c == '_')
                        || code[..k].ends_with("libc::")
                });
                if !bare {
                    continue;
                }
            }
            let allowed = match file {
                // THE wrapper: every raw signal syscall (libc::...) lives in its private `mod sys`; the rest of the file
                // calls those functions (`sys::...`) and names them in messages.
                "launch/guard.rs" => in_sys || !code.contains("libc::"),
                // cgroup.kill only on job cgroups this process created (JobCgroup / kill_path), and the probe's file list.
                "launch/cgroup.rs" => pat == "cgroup.kill",
                // The contract lane's fake child: kill of its own forked helpers (pid from its own fork, waited at once)
                // and of itself; never a group or a broadcast.
                "bin/rr_fake_child.rs" => pat == "kill(" && code.contains("libc::kill("),
                _ => false,
            };
            if !allowed {
                out.push(format!("{file}:{}: [{pat}] {}", i + 1, line.trim()));
            }
        }
    }
    out
}

/// RA m-3 (timing): stopping a job with many members outside the leader's group does not block the control loop for
/// N x the per-target bound: terminate() returns within a few seconds even with 24 individually signalled members.
#[test]
fn ra_m3_terminate_with_many_members_returns_quickly() {
    let _g = serial();
    require_namespace();
    let dir = TempDir::new("ra-m3");
    let mut l = launcher_with(|_| {});
    let script = "for i in $(seq 24); do setsid sleep 600 & done; exec sleep 600";
    let mut h = l.launch(&sh(dir.path(), "ram3", script)).unwrap();
    wait_until("members sampled", Duration::from_secs(60), || {
        let _ = h.poll();
        let n = l.sampler().latest().map_or(0, |s| {
            s.marked.iter().filter(|p| p.attempt_id == "ram3").count()
        });
        if n < 24 {
            println!(
                "RA m-3: {n} marked of ram3 sampled (campaign {:?} vs launcher {})",
                l.sampler().campaign_id(),
                l.config().campaign_id
            );
        }
        n >= 24
    });
    let t0 = Instant::now();
    h.terminate(Duration::from_secs(1), KillReason::Stop);
    let took = t0.elapsed();
    assert!(took < Duration::from_secs(5), "terminate took {took:?}");
    let r = drive(&mut h);
    assert!(r.verification.is_verified(), "{r:?}");
    println!("RA m-3: terminate with 24+ marked processes outside the group took {took:?}");
}

/// The marker type is shared with the scan; keep the import used.
#[allow(dead_code)]
fn _marker(c: &str) -> CampaignMarker {
    CampaignMarker::new(c, "x").unwrap()
}

/// Sanity: scan_campaign still lists nothing of a fresh id.
#[test]
fn scan_of_an_unused_id_is_empty() {
    let s = scan_campaign("rb-unused-id", 0, &BTreeSet::new()).unwrap();
    assert!(s.processes.is_empty());
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

fn site_cfg(root: &Path, cpu: usize) -> SiteConfig {
    let c = site_cfg_raw(root, cpu);
    make_site_files(&c, &[cpu]);
    c
}
