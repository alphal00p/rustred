//! gate14 (independent audit of procfix7 round 10, 639c26ce): adversarial children against the end-of-job proof
//! (G13c-M1: "never Verified while a re-forking descendant of the job runs").
//!
//! The job shape (every run): the job's leader L (marked, session sid) forks X. X sets PR_SET_CHILD_SUBREAPER, forks S
//! (a marked sleeper that stays in the job's session) and C (a marked relay: fork, the parent exits, the child setsid's
//! and repeats), then X setsid's and re-executes itself with an environment WITHOUT the marker: X is a live, marker-less
//! process of our uid outside every job session, i.e. "not ours" (never signalled), but the walk descends through it
//! (gate-A M-1: a marker-less process must not shield its descendants). Because X is a child subreaper, every hop of the
//! marked relay is re-parented to X, not to the supervisor. L exits after 300 ms.
//!
//! RR_G14T_XEXIT=never: X stays (it reaps the hops until the test's stop file ends the relay).
//! RR_G14T_XEXIT=s:     X exits as soon as S dies (S is killed by the hunt's kill step, i.e. after its fixed point): the
//!                      relay's hops are then re-parented to the supervisor while the proof passes run (an F2 scenario:
//!                      the fixed point (F1) cannot see the relay below X; only the passes' raw snapshots can).
//!
//! Variables (test side): RR_G14T_ITERS (default 6), RR_G14T_MODE fork | clonevm (default fork; clonevm: each hop is a
//! clone(CLONE_VM) of the previous one: hops of a few microseconds), RR_G14T_PAUSE_US (default 0), RR_G14T_XEXIT
//! (default never), RR_G14T_HOLD "<ms>x<n>" (test-build listing hold, default none), RR_G14T_BUDGET_S (hunt budget,
//! default the launcher's).
//!
//! Spawns and signals processes: ONLY inside a private pid namespace (rule L10): the test refuses unless it is pid 1.
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

#[path = "proc_common/mod.rs"]
mod common;

use common::*;
use rustred_campaign::launch::{
    CapPreference, ChildHandle, ChildLauncher, ChildProgress, FinalReport, RealChild, RealConfig,
    RealLauncher, RetireEvent, TrackingPreference,
};
use std::ffi::CString;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::sync::atomic::{AtomicI32, AtomicU32, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

fn launcher(budget: Option<Duration>) -> RealLauncher {
    let mut c = RealConfig {
        cap_preference: CapPreference::NoSystemd,
        tracking: TrackingPreference::ProcOnly,
        verify_timeout: Duration::from_secs(10),
        rehunt_every: Duration::from_millis(300),
        ..RealConfig::default()
    };
    if let Some(b) = budget {
        c.hunt_budget = b;
    }
    RealLauncher::with_config(sampler(), c)
}

fn poll_until_finished(h: &mut RealChild, t: Duration) -> Option<FinalReport> {
    let t0 = Instant::now();
    while t0.elapsed() < t {
        if let Ok(ChildProgress::Finished(r)) = h.poll() {
            return Some(r);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    None
}

fn relay_count(p: &Path) -> u64 {
    fs::read(p)
        .ok()
        .filter(|b| b.len() >= 8)
        .map(|b| u64::from_le_bytes(b[..8].try_into().unwrap()))
        .unwrap_or(0)
}

fn env_or(k: &str, d: &str) -> String {
    std::env::var(k).unwrap_or_else(|_| d.to_string())
}

// ------------------------------------------------------------------------------------------------ helper processes

/// Helper entry for re-executed modes (does nothing in a normal run).
#[test]
fn helper_entry() {
    match std::env::var("RR_PROC_HELPER").as_deref() {
        Ok("g14leader") => leader(),
        Ok("g14reaper") => reaper(),
        _ => {}
    }
}

static HOP_FD: AtomicI32 = AtomicI32::new(-1);
static HOP_STOP: AtomicUsize = AtomicUsize::new(0);
static HOP_PAUSE: AtomicU32 = AtomicU32::new(0);
static HOP_STACKS: AtomicUsize = AtomicUsize::new(0);
const NSTK: usize = 256;
const STK: usize = 64 * 1024;

/// One hop of the clonevm relay (runs on its own stack of the ring; shares the memory of the previous hop).
extern "C" fn hop(arg: *mut libc::c_void) -> libc::c_int {
    let i = arg as usize;
    // SAFETY: raw syscalls only; the stop path, fd and stack ring were set up before the first fork.
    unsafe {
        let stop = HOP_STOP.load(Ordering::Relaxed) as *const libc::c_char;
        if libc::access(stop, libc::F_OK) == 0 {
            return 0;
        }
        libc::setsid();
        let n = (i as u64).to_le_bytes();
        libc::pwrite(HOP_FD.load(Ordering::Relaxed), n.as_ptr().cast(), 8, 0);
        let p = HOP_PAUSE.load(Ordering::Relaxed);
        if p > 0 {
            libc::usleep(p);
        }
        let next = (i + 1) % NSTK;
        let base = HOP_STACKS.load(Ordering::Relaxed) as *mut u8;
        let top = base.add((next + 1) * STK);
        libc::clone(
            hop,
            top.cast(),
            libc::CLONE_VM | libc::SIGCHLD,
            (i + 1) as *mut libc::c_void,
        );
    }
    0
}

/// The fork relay of rb_c2 (async-signal-safe calls only; ends when the stop file exists).
unsafe fn relay_fork(fd: libc::c_int, stop: *const libc::c_char, pause: u32) -> ! {
    let mut i: u64 = 0;
    // SAFETY: single-threaded forked child; raw calls only.
    unsafe {
        loop {
            if libc::access(stop, libc::F_OK) == 0 {
                libc::_exit(0);
            }
            if libc::fork() != 0 {
                libc::_exit(0);
            }
            libc::setsid();
            i += 1;
            let b = i.to_le_bytes();
            libc::pwrite(fd, b.as_ptr().cast(), 8, 0);
            if pause > 0 {
                libc::usleep(pause);
            }
        }
    }
}

fn leader() {
    let mode = env_or("RR_G14_MODE", "fork");
    let pause: u32 = env_or("RR_G14_PAUSE_US", "0").parse().unwrap();
    let counter = std::env::var("RR_RELAY_COUNTER").unwrap();
    let stop = std::env::var("RR_G14_STOP").unwrap();
    let xexit = env_or("RR_G14_XEXIT", "never");
    // Everything X and C need is prepared BEFORE the fork (the children use async-signal-safe calls only).
    let exe = CString::new(std::env::current_exe().unwrap().as_os_str().as_bytes()).unwrap();
    let argv_s: Vec<CString> = [
        exe.to_bytes(),
        b"helper_entry".as_slice(),
        b"--exact",
        b"--nocapture",
        b"--test-threads=1",
    ]
    .iter()
    .map(|a| CString::new(*a).unwrap())
    .collect();
    let mut argv: Vec<*const libc::c_char> = argv_s.iter().map(|c| c.as_ptr()).collect();
    argv.push(std::ptr::null());
    let env_s: Vec<CString> = [
        "RR_PROC_HELPER=g14reaper".to_string(),
        format!("RR_G14_STOP={stop}"),
        format!("RR_G14_XEXIT={xexit}"),
    ]
    .into_iter()
    .map(|s| CString::new(s).unwrap())
    .collect();
    let mut spid_buf: Vec<u8> = b"RR_G14_SPID=0000000000\0".to_vec();
    let mut envp: Vec<*const libc::c_char> = env_s.iter().map(|c| c.as_ptr()).collect();
    envp.push(spid_buf.as_ptr().cast());
    envp.push(std::ptr::null());
    let cpath = CString::new(counter).unwrap();
    let cstop = CString::new(stop).unwrap();
    // SAFETY: open with a valid C string.
    let fd = unsafe { libc::open(cpath.as_ptr(), libc::O_WRONLY | libc::O_CREAT, 0o600) };
    assert!(fd >= 0);
    if mode == "clonevm" {
        // SAFETY: an anonymous private mapping for the hop stacks.
        let m = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                NSTK * STK,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            )
        };
        assert_ne!(m, libc::MAP_FAILED);
        HOP_STACKS.store(m as usize, Ordering::SeqCst);
        HOP_FD.store(fd, Ordering::SeqCst);
        HOP_STOP.store(cstop.as_ptr() as usize, Ordering::SeqCst);
        HOP_PAUSE.store(pause, Ordering::SeqCst);
    }
    let spid_ptr = spid_buf.as_mut_ptr();
    // SAFETY: fork; the children use only async-signal-safe calls (prctl, fork, setsid, clone, access, pwrite, usleep,
    // execve, _exit) on memory prepared above.
    unsafe {
        let x = libc::fork();
        if x == 0 {
            libc::prctl(libc::PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0);
            let s = libc::fork();
            if s == 0 {
                loop {
                    libc::pause();
                }
            }
            let c = libc::fork();
            if c == 0 {
                if mode.as_bytes() == b"clonevm" {
                    let base = HOP_STACKS.load(Ordering::Relaxed) as *mut u8;
                    libc::clone(
                        hop,
                        base.add(STK).cast(),
                        libc::CLONE_VM | libc::SIGCHLD,
                        std::ptr::null_mut(),
                    );
                    libc::_exit(0);
                }
                relay_fork(fd, cstop.as_ptr(), pause);
            }
            libc::setsid();
            // S's pid into the digits of RR_G14_SPID= (fixed width, in place)
            let mut v = s.max(0) as u32;
            for k in 0..10 {
                *spid_ptr.add(12 + 9 - k) = b'0' + (v % 10) as u8;
                v /= 10;
            }
            libc::execve(exe.as_ptr(), argv.as_ptr(), envp.as_ptr());
            libc::_exit(127);
        }
        assert!(x > 0);
    }
    std::thread::sleep(Duration::from_millis(300));
}

/// X after its exec: no marker in its environment; reaps the re-parented hops; exits on ECHILD, or (XEXIT=s) as soon as
/// S has died.
fn reaper() {
    let xexit = env_or("RR_G14_XEXIT", "never");
    let spid: i32 = env_or("RR_G14_SPID", "0").parse().unwrap_or(0);
    loop {
        let mut st = 0;
        // SAFETY: waitpid for any child of this process.
        let r = unsafe { libc::waitpid(-1, &mut st, 0) };
        if r < 0 {
            if std::io::Error::last_os_error().raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            break;
        }
        if xexit == "s" && r == spid {
            break;
        }
    }
}

// ------------------------------------------------------------------------------------------------ the test

#[derive(Default, Debug)]
struct Tally {
    violations: Vec<String>,
    verified_clean: u32,
    unverified_then_cleared_clean: u32,
}

fn one_run(l: &mut RealLauncher, dir: &Path, i: usize, t: &mut Tally) {
    let mode = env_or("RR_G14T_MODE", "fork");
    let pause = env_or("RR_G14T_PAUSE_US", "0");
    let xexit = env_or("RR_G14T_XEXIT", "never");
    let hold = env_or("RR_G14T_HOLD", "");
    let label = format!("g14r{i}");
    let cf = dir.join(format!("{label}.count"));
    let stop = dir.join(format!("{label}.stop"));
    let mut s = helper_spec(dir, &label, "helper_entry", "g14leader");
    s.env.push(("RR_G14_MODE".into(), mode.clone().into()));
    s.env.push(("RR_G14_PAUSE_US".into(), pause.clone().into()));
    s.env.push(("RR_RELAY_COUNTER".into(), cf.clone().into()));
    s.env.push(("RR_G14_STOP".into(), stop.clone().into()));
    s.env.push(("RR_G14_XEXIT".into(), xexit.clone().into()));
    if let Some((ms, n)) = hold.split_once('x') {
        rustred_campaign::resources::set_listing_hold(ms.parse().unwrap(), n.parse().unwrap());
    }
    let t0 = Instant::now();
    let mut h = l.launch(&s).unwrap();
    let r = poll_until_finished(&mut h, Duration::from_secs(120)).expect("a report");
    let first = t0.elapsed();
    let c1 = relay_count(&cf);
    std::thread::sleep(Duration::from_secs(1));
    let c2 = relay_count(&cf);
    rustred_campaign::resources::set_listing_hold(0, 0);
    let what;
    if r.verification.is_verified() {
        if c2 > c1 {
            what = format!("VIOLATION: Verified while the relay runs ({c1} -> {c2} hops)");
            t.violations.push(format!(
                "run {i}: first report {:?} ({c1} -> {c2})",
                r.verification
            ));
        } else {
            what = format!("Verified, relay stopped ({c1} hops)");
            t.verified_clean += 1;
        }
    } else {
        assert!(l.retire(h, None).is_none());
        let t1 = Instant::now();
        let after = loop {
            let ev = l.tick();
            if let Some(RetireEvent::Cleared { after, .. }) = ev
                .into_iter()
                .find(|e| matches!(e, RetireEvent::Cleared { label: x, .. } if *x == label))
            {
                break after;
            }
            assert!(
                t1.elapsed() < Duration::from_secs(180),
                "not cleared within 180 s: {:?}",
                l.quarantined()
            );
            std::thread::sleep(Duration::from_millis(20));
        };
        let c3 = relay_count(&cf);
        std::thread::sleep(Duration::from_secs(1));
        let c4 = relay_count(&cf);
        if c4 > c3 {
            what =
                format!("VIOLATION: Unverified, then CLEARED while the relay runs ({c3} -> {c4})");
            t.violations.push(format!(
                "run {i}: cleared after {after:?} while running ({c3} -> {c4})"
            ));
        } else {
            what = format!("Unverified, then cleared after {after:?}, relay stopped ({c3} hops)");
            t.unverified_then_cleared_clean += 1;
        }
    }
    eprintln!(
        "G14 run {i} mode {mode} pause {pause} us xexit {xexit} hold '{hold}': first report after {first:?} \
         ({:?}; killed {}; unattributed {}): {what}",
        r.verification,
        r.leftovers_killed.len(),
        r.unattributed_children.len()
    );
    // end the relay (cooperative: the stop file), then X (ECHILD); nothing is signalled
    fs::write(&stop, b"").unwrap();
    let t2 = Instant::now();
    loop {
        let a = relay_count(&cf);
        std::thread::sleep(Duration::from_millis(300));
        if a == relay_count(&cf) {
            break;
        }
        assert!(t2.elapsed() < Duration::from_secs(30), "relay did not stop");
    }
    std::thread::sleep(Duration::from_millis(300));
}

/// G14 (item 1): a marked re-forking relay below a marker-less CHILD SUBREAPER of the job. Expected by the proof's
/// contract: never Verified (nor cleared) while the relay runs.
#[test]
#[ignore = "gate14 audit test: run explicitly (--ignored --exact) inside a private pid namespace"]
fn g14_marked_relay_below_a_markerless_subreaper_is_never_verified_while_it_runs() {
    if std::process::id() != 1 {
        eprintln!(
            "SKIPPED: spawns and signals processes: only inside a private pid namespace (rule L10)"
        );
        return;
    }
    let _g = serial();
    let dir = TempDir::new("g14-subreaper");
    let iters: usize = env_or("RR_G14T_ITERS", "6").parse().unwrap();
    let budget = std::env::var("RR_G14T_BUDGET_S")
        .ok()
        .map(|s| Duration::from_secs(s.parse().unwrap()));
    let mut l = launcher(budget);
    let mut t = Tally::default();
    for i in 0..iters {
        one_run(&mut l, dir.path(), i, &mut t);
    }
    eprintln!(
        "G14 SUMMARY mode {} pause {} xexit {} hold '{}': runs {iters}, violations {}, verified (relay stopped) {}, \
         unverified then cleared (relay stopped) {}",
        env_or("RR_G14T_MODE", "fork"),
        env_or("RR_G14T_PAUSE_US", "0"),
        env_or("RR_G14T_XEXIT", "never"),
        env_or("RR_G14T_HOLD", ""),
        t.violations.len(),
        t.verified_clean,
        t.unverified_then_cleared_clean
    );
    assert!(t.violations.is_empty(), "G14: {:?}", t.violations);
}
