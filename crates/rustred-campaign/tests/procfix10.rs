//! Lane procfix7 round 6 (gate10, WS/notes/implementer.gate10.md): G10-B1 (a TRUNCATED /proc listing taken as
//! complete), M10-1 (vanished marked processes of every sweep), the job-scope refusal, m10-5 (a child that can never be
//! polled).
//!
//! The listing rule (src/resources/procfs.rs `pids` / `pids_proven` / `tasks`): a listing is evidence of ABSENCE only
//! with a positive completeness proof: its sentinels (this process, pid 1, the caller's known live processes) and an
//! independent second source (the children-list walk from pid 1 and from this process); and every pid the ledger
//! recorded is proven dead by its own pid (the sweeps' seeds), never by its absence from a listing.
//!
//! MUTATION TESTS: both enumerations of the proc root listing end after k entries IN AGREEMENT (test-build fault
//! `proc_trunc_at:<k+1>`), for every k of the small tree of the test namespace, for a crashed session with a live
//! marked daemon and for a live session whose job starts one: never exit 0 with the marked daemon alive.
//!
//! Spawns processes and sends signals: ONLY inside a private pid namespace (`LIGHT_PIDNS=1 light_build.sh ...`, rule L10).
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

mod closeloop_support;
#[path = "sup_common/mod.rs"]
mod common;

use common::*;
use std::os::fd::{AsRawFd, OwnedFd};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

fn in_ns() -> bool {
    if rustred_campaign::launch::in_private_namespace() {
        return true;
    }
    eprintln!(
        "SKIPPED: spawns and signals processes: only inside a private pid namespace (rule L10)"
    );
    false
}

fn path_env() -> std::ffi::OsString {
    std::env::var_os("PATH").unwrap_or_default()
}

fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, body).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

fn alive(pid: u32) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .and_then(|s| s.rfind(')').map(|i| !s[i + 2..].starts_with('Z')))
        .unwrap_or(false)
}

/// Test code: SIGKILL one exact pid of ours (the shell's builtin).
fn kill_pid(pid: u32) {
    let _ = Command::new("sh")
        .args(["-c", &format!("kill -KILL {pid} 2>/dev/null")])
        .env("PATH", path_env())
        .status();
}

/// A spec whose first job runs `cmd` (through a wrapper of the fake child) once.
fn spec_with(t: &Tmp, cmd: &str, case_ms: [u64; 2]) -> PathBuf {
    let rec = t.path.join("started.rec");
    let wrap = script(
        &t.path,
        "wrap.sh",
        &format!(
            "#!/bin/sh\nif [ \"$1\" = job ] && [ ! -e {rec} ]; then\n  : > {rec}\n  {cmd}\nfi\nexec {fake} \"$@\"\n",
            rec = rec.display(),
            fake = FAKE
        ),
    );
    let mut v = spec(
        t,
        serde_json::json!({"items": 3, "nodes": 1, "rungs": 1, "seed": 5, "case_ms": case_ms,
            "p_crash": 0.0, "p_mem": 0.0, "p_norule": 0.0, "deps": 0.0}),
        serde_json::json!({"max_jobs": 2, "unproven_end_limit_s": 20}),
    );
    v["programs"]["rr_fake_child"]["path"] = serde_json::json!(wrap);
    write_spec(t, "spec.json", &v)
}

fn run(sp: &Path, d: &Path, env: &[(&str, &str)], limit: Duration) -> i32 {
    let mut c: Child = start(sp, d, &[], env);
    wait(&mut c, limit)
}

/// One crashed-session case: session 1's first job leaves a marked daemon, session 1 is SIGKILLed, session 2 runs with
/// both enumerations of every proc root listing ending after k entries. (exit code, daemon alive after)
fn trunc_crashed(k: usize) -> (i32, bool) {
    let t = Tmp::new(&format!("pf10-tc-{k}"));
    let dpf = t.path.join("daemon.pid");
    let sp = spec_with(
        &t,
        &format!(
            "( setsid sleep 600 </dev/null >/dev/null 2>&1 & echo $! > {p}.tmp; mv {p}.tmp {p} )",
            p = dpf.display()
        ),
        [800, 1200],
    );
    let d = t.cdir("c");
    let mut s1 = start(&sp, &d, &[], &[]);
    wait_until("the daemon", Duration::from_secs(300), || dpf.exists());
    std::thread::sleep(Duration::from_millis(300));
    signal(&s1, libc::SIGKILL);
    let _ = s1.wait();
    let dp: u32 = std::fs::read_to_string(&dpf)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let fp = format!("proc_trunc_at:{}", k + 1);
    let code = run(
        &sp,
        &d,
        &[("RR_CAMPAIGN_FAILPOINT", &fp)],
        Duration::from_secs(900),
    );
    let a = alive(dp);
    if a {
        kill_pid(dp);
    }
    (code, a)
}

/// One live-session case: the job starts a marked fork chain (200 ms per hop, a FIFO holder: its liveness is read
/// atomically) AFTER the supervisor (higher pids), the session runs with truncated listings from the start.
fn trunc_live(k: usize) -> (i32, bool) {
    let t = Tmp::new(&format!("pf10-tl-{k}"));
    let ch = Fifo::new(&t.path);
    let sp = spec_with(
        &t,
        &format!(
            "{FAKE} churn-procs {} 200000 </dev/null >/dev/null 2>&1",
            ch.path.display()
        ),
        [3000, 4000],
    );
    let d = t.cdir("c");
    let fp = format!("proc_trunc_at:{}", k + 1);
    let code = run(
        &sp,
        &d,
        &[("RR_CAMPAIGN_FAILPOINT", &fp)],
        Duration::from_secs(900),
    );
    let a = ch.alive_settled();
    drop(ch); // the chain ends at its next write (EPIPE)
    (code, a)
}

/// Test code, ONLY while this test binary is pid 1 of its private namespace (LIGHT_PIDNS): reaps ZOMBIE orphans
/// re-parented to it whose command is `comm` (the incarnations of a crashed session's fork chain). An init that never
/// reaps them fills the pid limit (a 0-us chain left 629,136 zombies and made every fork fail with EAGAIN). Reaps nothing
/// else: the test's own children (the supervisors it spawns) have another command; nothing is ever signalled.
struct OrphanReaper {
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    h: Option<std::thread::JoinHandle<()>>,
}

impl OrphanReaper {
    fn start(comm: &'static str) -> OrphanReaper {
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let s2 = stop.clone();
        let h = (std::process::id() == 1).then(|| {
            std::thread::spawn(move || {
                while !s2.load(std::sync::atomic::Ordering::Relaxed) {
                    if let Ok(rd) = std::fs::read_dir("/proc") {
                        for e in rd.flatten() {
                            let Some(pid) =
                                e.file_name().to_str().and_then(|n| n.parse::<i32>().ok())
                            else {
                                continue;
                            };
                            let Ok(st) = std::fs::read_to_string(format!("/proc/{pid}/stat"))
                            else {
                                continue;
                            };
                            let (Some(a), Some(b)) = (st.find('('), st.rfind(')')) else {
                                continue;
                            };
                            let rest: Vec<&str> = st[b + 2..].split_whitespace().collect();
                            if &st[a + 1..b] == comm
                                && rest.first() == Some(&"Z")
                                && rest.get(1) == Some(&"1")
                            {
                                // SAFETY: waitpid on one exact pid with a null status pointer
                                unsafe { libc::waitpid(pid, std::ptr::null_mut(), libc::WNOHANG) };
                            }
                        }
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
            })
        });
        OrphanReaper { stop, h }
    }
}

impl Drop for OrphanReaper {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(h) = self.h.take() {
            let _ = h.join();
        }
    }
}

/// Runs `case` for every k of `ks`, four campaigns at a time; Err(failures) when one ended 0 with its marked process
/// alive.
fn every_k(what: &str, ks: &[usize], case: fn(usize) -> (i32, bool)) {
    let mut fails = Vec::new();
    let mut tally: std::collections::BTreeMap<i32, usize> = Default::default();
    for chunk in ks.chunks(4) {
        let hs: Vec<_> = chunk
            .iter()
            .map(|&k| std::thread::spawn(move || (k, case(k))))
            .collect();
        for h in hs {
            let (k, (code, a)) = h.join().unwrap();
            *tally.entry(code).or_default() += 1;
            if code == 0 && a {
                fails.push(format!("k {k}: exit 0 with the marked process alive"));
            }
        }
    }
    eprintln!(
        "PF10 {what}, k {:?}: exit codes {tally:?}; {} failure(s)",
        ks,
        fails.len()
    );
    assert!(fails.is_empty(), "{what}: {fails:?}");
}

/// The k of a mutation run: every k of 0..=16 (`all`; the namespace's tree during a session is smaller: pid 1, the
/// supervisors, the jobs and their wrappers, the daemon or chain), else every third.
fn ks(all: bool) -> Vec<usize> {
    (0..=16).filter(|k| all || k % 3 == 0).collect()
}

/// G10-B1 mutation, crashed session (suite: every third k; every k: the ignored variant): never exit 0 with the crashed
/// session's live marked daemon.
#[test]
fn g10_b1_truncated_root_listing_crashed_session() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    every_k("truncation crashed", &ks(false), trunc_crashed);
}

#[test]
#[ignore = "every k (17 campaigns of up to 3 min each, 4 at a time); run explicitly"]
fn g10_b1_truncated_root_listing_crashed_session_every_k() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    every_k("truncation crashed (every k)", &ks(true), trunc_crashed);
}

/// The CONTROL of the truncation tests: the same crashed / live scenarios with a k beyond any tree size (nothing is
/// truncated): the crashed session's daemon is killed and the run ends 0; the live session's chain is killed. It shows
/// that the refusals / unproven ends of the truncated runs come from the truncation alone.
#[test]
fn g10_b1_truncation_control_without_truncation() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let _r = OrphanReaper::start("rr_fake_child");
    let (code, a) = trunc_crashed(100_000);
    eprintln!("PF10 control crashed (no truncation): exit {code}, daemon alive {a}");
    assert!(
        code == 0 && !a,
        "control crashed: exit {code}, daemon alive {a}"
    );
    let (code, a) = trunc_live(100_000);
    eprintln!("PF10 control live (no truncation): exit {code}, chain alive {a}");
    assert!(!a, "control live: the chain is killed (exit {code})");
    assert!(code == 0 || code == ATTENTION, "control live: exit {code}");
}

/// The test's end of a churner's FIFO (procfix9's: EOF = every writer is gone; the drain is bounded).
struct Fifo {
    fd: OwnedFd,
    path: PathBuf,
}

impl Fifo {
    fn new(dir: &Path) -> Fifo {
        let path = dir.join("hold.fifo");
        let c = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
        // SAFETY: mkfifo / open with a NUL-terminated path.
        assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0, "mkfifo");
        // procfix7 round 6: O_CLOEXEC: the supervisors this test starts (and their jobs, and the chain) must NOT inherit
        // the read end: a chain holding a reader of its own FIFO never gets EPIPE and outlives the test's clean-up
        let fd = unsafe {
            libc::open(
                c.as_ptr(),
                libc::O_RDONLY | libc::O_NONBLOCK | libc::O_CLOEXEC,
            )
        };
        assert!(fd >= 0, "open the fifo");
        // SAFETY: a descriptor we just opened.
        let fd = unsafe { <OwnedFd as std::os::fd::FromRawFd>::from_raw_fd(fd) };
        Fifo { fd, path }
    }
    fn read(&self) -> std::io::Result<usize> {
        let mut b = [0u8; 4096];
        // SAFETY: a read into our stack buffer from our descriptor.
        let n = unsafe { libc::read(self.fd.as_raw_fd(), b.as_mut_ptr().cast(), b.len()) };
        if n < 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(n as usize)
        }
    }
    fn alive(&self) -> bool {
        for _ in 0..256 {
            match self.read() {
                Ok(0) => return false,
                Ok(_) => continue,
                Err(e) if e.raw_os_error() == Some(libc::EAGAIN) => return true,
                Err(e) if e.raw_os_error() == Some(libc::EINTR) => continue,
                Err(e) => panic!("fifo read: {e}"),
            }
        }
        true
    }
    fn alive_settled(&self) -> bool {
        let t0 = Instant::now();
        while t0.elapsed() < Duration::from_secs(2) {
            if !self.alive() {
                return false;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        self.alive()
    }
}

/// G10-B1 mutation, LIVE session: the job's marked chain starts after the supervisor (higher pids), so a truncation
/// between them keeps the sentinels (this process, pid 1) and hides the chain: only the children-list walk (from this
/// process: the chain is a descendant of the subreaper) or the own-tree proof reveals it. Never exit 0 with it alive.
#[test]
fn g10_b1_truncated_root_listing_live_session() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let _r = OrphanReaper::start("rr_fake_child");
    every_k("truncation live", &ks(false), trunc_live);
}

#[test]
#[ignore = "every k (17 campaigns of up to 3 min each, 4 at a time); run explicitly"]
fn g10_b1_truncated_root_listing_live_session_every_k() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let _r = OrphanReaper::start("rr_fake_child");
    every_k("truncation live (every k)", &ks(true), trunc_live);
}

/// M10-1 (one rule for every sweep): a crashed session's marked fork chain with 5 ms per hop is seen by the recovery
/// sweep; a sweep in which a marked process vanished is never proven, and the end stays unproven until a LATER sweep
/// seeded with those pids is proven. Never exit 0 with the chain alive; an exit 0 after a recorded vanish carries the
/// resolution note.
#[test]
fn g10_m10_1_vanished_marked_processes_keep_the_end_unproven() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let _r = OrphanReaper::start("rr_fake_child");
    let t = Tmp::new("pf10-m10-1");
    let ch = Fifo::new(&t.path);
    let sp = spec_with(
        &t,
        &format!(
            "{FAKE} churn-procs {} 5000 </dev/null >/dev/null 2>&1",
            ch.path.display()
        ),
        [800, 1200],
    );
    let d = t.cdir("c");
    let mut s1 = start(&sp, &d, &[], &[]);
    wait_until(
        "the chain",
        Duration::from_secs(300),
        || matches!(ch.read(), Ok(n) if n > 0),
    );
    std::thread::sleep(Duration::from_millis(300));
    signal(&s1, libc::SIGKILL);
    let _ = s1.wait();
    assert!(ch.alive(), "the chain runs after the crash");
    let code = run(&sp, &d, &[], Duration::from_secs(900));
    let alive = ch.alive_settled();
    let vanished = notes(&d, "sweep_vanished {");
    let resolved = notes(&d, "sweep_vanished_resolved");
    eprintln!(
        "PF10 m10-1: exit {code}, chain alive {alive}, vanished notes {}, resolved notes {}",
        vanished.len(),
        resolved.len()
    );
    drop(ch);
    // the guaranteed property (RUNBOOK 2.10): a recorded vanish is never followed by exit 0 unless an atomic proof
    // resolved it (none exists for a crashed session's chain in a namespace: no resolution note is possible here)
    if !vanished.is_empty() {
        assert_ne!(code, 0, "exit 0 after a recorded vanish: {}", log_tail(&d));
        assert!(resolved.is_empty(), "{resolved:?}");
    }
    // a chain NO sweep ever saw can end 0 alive: the documented residual of a namespace run without a job scope
    // (RUNBOOK 2.7, gate10's table: it grows with the host's load); reported, the scope closes it on the host
    if code == 0 && alive {
        eprintln!(
            "PF10 m10-1: RESIDUAL (no sweep saw the chain; marker_only in a namespace): exit 0 with the chain alive"
        );
    }
}

/// The job-scope ruling: the marker-only override is accepted only inside a private pid namespace; on the host
/// (test-build failpoint `namespace_is_host:1`) it is refused with a typed end (1), and nothing is launched.
#[test]
fn g10_the_marker_only_override_is_refused_on_the_host() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("pf10-scope");
    let sp = spec_with(&t, "true", [100, 200]);
    let d = t.cdir("c");
    let code = run(
        &sp,
        &d,
        &[("RR_CAMPAIGN_FAILPOINT", "namespace_is_host:1")],
        Duration::from_secs(120),
    );
    assert_eq!(code, 1, "{}", log_tail(&d));
    assert!(
        log_tail(&d).contains("accepted only inside a private pid namespace"),
        "{}",
        log_tail(&d)
    );
    assert!(!t.path.join("started.rec").exists(), "nothing launched");
}

/// gate10 m10-5: a child that can never be polled (every waitid / pidfd read failing: test-build failpoint
/// `poll_fails:1`) ends typed (exit 70) after a bound, never an endless loop of "poll" lines.
#[test]
fn g10_m10_5_a_child_that_cannot_be_polled_is_a_typed_end() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("pf10-poll");
    let sp = spec_with(&t, "true", [20_000, 30_000]);
    let d = t.cdir("c");
    let t0 = Instant::now();
    let code = run(
        &sp,
        &d,
        &[("RR_CAMPAIGN_FAILPOINT", "poll_fails:1")],
        Duration::from_secs(300),
    );
    eprintln!("PF10 m10-5: exit {code} after {:?}", t0.elapsed());
    assert_eq!(code, 70, "{}", log_tail(&d));
    assert!(
        log_tail(&d).contains("consecutive failed polls"),
        "{}",
        log_tail(&d)
    );
    // procfix7 round 15 (gate16 m16-poll-stop): a stop decision, the ledger usable: every delivered signal (the drain's,
    // the exit's) has its ledger record, and the progress states the ledger's end (gate 16: 8 delivered, 0 recorded)
    let dec = notes(&d, "stop_decided ");
    assert_eq!(dec.len(), 1, "{dec:?}");
    assert!(dec[0].contains("\"exit\":70"), "{dec:?}");
    let intents = std::fs::read_to_string(d.join("log/signal-intents.jsonl")).unwrap_or_default();
    let delivered: Vec<String> = intents
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter(|v| v["kind"] == "done" && v["result"] == "delivered")
        .filter_map(|v| v["id"].as_str().map(str::to_string))
        .collect();
    let bys: Vec<String> = records(&d)
        .into_iter()
        .filter_map(|r| match r.event {
            rustred_campaign::ledger::Event::SignalSent { by, .. } => Some(by),
            _ => None,
        })
        .collect();
    let missing: Vec<&String> = delivered
        .iter()
        .filter(|id| !bys.iter().any(|b| b.contains(&format!("[intent {id}]"))))
        .collect();
    eprintln!(
        "PF10 m10-5: {} delivered, {} recorded",
        delivered.len(),
        bys.len()
    );
    assert!(
        !delivered.is_empty(),
        "the drain signalled the unobservable jobs"
    );
    assert!(missing.is_empty(), "delivered, not recorded: {missing:?}");
    assert!(
        !std::fs::read_to_string(d.with_extension("log"))
            .unwrap_or_default()
            .contains("not recorded")
    );
    assert_eq!(verify(&d), 0, "verify after the poll-failure stop");
}

/// A crashed session whose first job left a marked daemon (setsid sleep): (spec, cdir, daemon pid).
fn crashed_with_daemon(t: &Tmp) -> (PathBuf, PathBuf, u32) {
    crashed_with_daemon_lim(t, None)
}

/// As [`crashed_with_daemon`], with `resources.unproven_end_limit_s` set in the spec BEFORE session 1 (a spec changed
/// after the campaign's creation is refused).
fn crashed_with_daemon_lim(t: &Tmp, unproven_s: Option<u64>) -> (PathBuf, PathBuf, u32) {
    let dpf = t.path.join("daemon.pid");
    let sp = spec_with(
        t,
        &format!(
            "( setsid sleep 600 </dev/null >/dev/null 2>&1 & echo $! > {p}.tmp; mv {p}.tmp {p} )",
            p = dpf.display()
        ),
        [800, 1200],
    );
    if let Some(u) = unproven_s {
        let mut v: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&sp).unwrap()).unwrap();
        v["resources"]["unproven_end_limit_s"] = serde_json::json!(u);
        std::fs::write(&sp, serde_json::to_vec_pretty(&v).unwrap()).unwrap();
    }
    let d = t.cdir("c");
    let mut s1 = start(&sp, &d, &[], &[]);
    wait_until("the daemon", Duration::from_secs(300), || dpf.exists());
    std::thread::sleep(Duration::from_millis(300));
    signal(&s1, libc::SIGKILL);
    let _ = s1.wait();
    let dp: u32 = std::fs::read_to_string(&dpf)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(alive(dp), "the daemon runs after the crash");
    (sp, d, dp)
}

/// gate10 mutation M4 (the vanished rule pinned by no test): the crashed session's marked daemon is reported by EVERY
/// sweep as seen alive and gone before its stop (test-build hook `sweep_vanish_pid:<pid>`; it stays alive): the
/// recovery sweep is not proven, the vanish keeps the end unproven, the final sweep cannot resolve it (it vanishes
/// again): exit 8 naming it (note `final_sweep_vanished`), never exit 0. A build that ignores vanishes ends 0 with the
/// daemon alive here; a build whose vanished_check is off ends 8 without the note (caught).
#[test]
fn g10_m4_a_vanished_marked_process_is_never_a_clean_end() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("pf10-m4");
    let (sp, d, dp) = crashed_with_daemon(&t);
    let fp = format!("sweep_vanish_pid:{dp}");
    let code = run(
        &sp,
        &d,
        &[("RR_CAMPAIGN_FAILPOINT", &fp)],
        Duration::from_secs(900),
    );
    let a = alive(dp);
    eprintln!("PF10 m4: exit {code}, daemon alive {a}");
    assert!(
        a,
        "a vanished process is never signalled (the hook leaves it alive)"
    );
    assert_eq!(code, ATTENTION, "{}", log_tail(&d));
    assert!(
        !notes(&d, "sweep_vanished {").is_empty(),
        "the vanish is recorded: {}",
        log_tail(&d)
    );
    // the vanished rule's own end (not only the generic unproven sweep): pins vanished_check (gate10 mutation M4)
    let fv = notes(&d, "final_sweep_vanished");
    assert!(
        fv.iter().any(|n| n.contains(&format!("{dp}"))),
        "the end names the unresolved vanished pid (note final_sweep_vanished): {fv:?} {}",
        log_tail(&d)
    );
    assert!(
        notes(&d, "sweep_vanished_resolved").is_empty(),
        "a pid that vanishes in every sweep is never resolved: {}",
        log_tail(&d)
    );
    assert!(
        log_tail(&d).contains(&format!("{dp}")),
        "the exit-8 message names it: {}",
        log_tail(&d)
    );
    let _ = run(&sp, &d, &[], Duration::from_secs(600));
    if alive(dp) {
        kill_pid(dp);
    }
}

/// gate10 mutation M5 (the own-tree proof pinned by no test): a live session's job leaves an UNMARKED descendant
/// (env -i, setsid: no marker, its own session) that the subreaper supervisor inherits. No marker sweep and no hidden-
/// descendants check sees it; only the kernel's wait answer (a child still present) does: exit 8 naming it (note
/// descendants_alive), never exit 0.
#[test]
fn g10_m5_an_unmarked_descendant_left_behind_is_never_a_clean_end() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("pf10-m5");
    let upf = t.path.join("unmarked.pid");
    let sp = spec_with(
        &t,
        &format!(
            "( env -i PATH=\"$PATH\" setsid sleep 600 </dev/null >/dev/null 2>&1 & echo $! > {p}.tmp; mv {p}.tmp {p} )",
            p = upf.display()
        ),
        [500, 800],
    );
    let d = t.cdir("c");
    let code = run(&sp, &d, &[], Duration::from_secs(900));
    let up: u32 = std::fs::read_to_string(&upf)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let a = alive(up);
    eprintln!("PF10 m5: exit {code}, unmarked descendant {up} alive {a}");
    if a {
        kill_pid(up);
    }
    assert!(a, "an unmarked process is never signalled");
    assert_eq!(code, ATTENTION, "{}", log_tail(&d));
    assert!(
        !notes(&d, "descendants_alive").is_empty(),
        "the own-tree proof names it: {}",
        log_tail(&d)
    );
}

/// gate10 mutation M6 (gate9 M9-1 pinned by no test): when the recovery cannot tell whether it runs in a private
/// namespace (test-build failpoint `uid_map_unreadable:1`), a recorded job scope of the crashed session that it would
/// have to stop is NOT skipped silently: note `scope_backstop_unchecked` naming the unit, and the end is unproven (exit
/// 8), never 0. Session 1 runs with `record_scope_unit:1` (test builds: the launch record names a scope unit; the launch
/// itself is unchanged, no systemd); nothing is ever sent to systemd here (the backstop only acts on a known host).
#[test]
fn g10_m6_an_unknown_namespace_never_skips_the_scope_backstop_silently() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("pf10-m6");
    let dpf = t.path.join("daemon.pid");
    let sp = spec_with(
        &t,
        &format!(
            "( setsid sleep 600 </dev/null >/dev/null 2>&1 & echo $! > {p}.tmp; mv {p}.tmp {p} ); sleep 5",
            p = dpf.display()
        ),
        [800, 1200],
    );
    let d = t.cdir("c");
    let mut s1 = start(
        &sp,
        &d,
        &[],
        &[("RR_CAMPAIGN_FAILPOINT", "record_scope_unit:1")],
    );
    wait_until("the daemon", Duration::from_secs(300), || dpf.exists());
    std::thread::sleep(Duration::from_millis(500));
    signal(&s1, libc::SIGKILL);
    let _ = s1.wait();
    let dp: u32 = std::fs::read_to_string(&dpf)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let code = run(
        &sp,
        &d,
        &[("RR_CAMPAIGN_FAILPOINT", "uid_map_unreadable:1")],
        Duration::from_secs(900),
    );
    let a = alive(dp);
    if a {
        kill_pid(dp);
    }
    let unchecked = notes(&d, "scope_backstop_unchecked");
    eprintln!(
        "PF10 m6: exit {code}, daemon alive {a}, scope_backstop_unchecked notes {}",
        unchecked.len()
    );
    assert!(
        unchecked.iter().any(|n| n.contains("rr-")),
        "the recorded scope that could not be stopped is named: {unchecked:?} {}",
        log_tail(&d)
    );
    assert_eq!(code, ATTENTION, "{}", log_tail(&d));
    assert!(
        notes(&d, "scope_killed").is_empty(),
        "nothing was sent to systemd for the recorded scope (namespace unknown): {}",
        log_tail(&d)
    );
}

fn site_index(name: &str) -> usize {
    rustred_campaign::fsx::SITES
        .iter()
        .position(|s| *s == name)
        .unwrap_or_else(|| panic!("no fsx site {name}"))
        + 1
}

/// G10-B1 for a DIRECTORY listing that decides absence (site `guard.fallback`: which earlier sessions' tmpfs signal
/// intent logs are merged): the listing ends early WITHOUT an error (`fsx_trunc_site` / `fsx_trunc_at`, test builds),
/// for every k of the small directory. The crashed session's log (created at its start) is a RECORDED session of this
/// boot: probed BY NAME, merged, and the hidden listing keeps the end unproven (note `signal_intents_unreadable`
/// "did not show it", exit 8). Never exit 0 with the crashed session's marked daemon alive.
#[test]
fn g10_b1_truncated_fallback_listing_never_drops_a_recorded_session_log() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let mut seen_hidden = false;
    for n in 1..=3usize {
        let t = Tmp::new(&format!("pf10-fb-{n}"));
        let (sp, d, dp) = crashed_with_daemon(&t);
        let fp = format!(
            "fsx_trunc_site:{},fsx_trunc_at:{n}",
            site_index("guard.fallback")
        );
        let code = run(
            &sp,
            &d,
            &[("RR_CAMPAIGN_FAILPOINT", &fp)],
            Duration::from_secs(900),
        );
        let a = alive(dp);
        if a {
            kill_pid(dp);
        }
        let hidden = notes(&d, "signal_intents_unreadable")
            .iter()
            .any(|n| n.contains("did not show it"));
        seen_hidden |= hidden;
        eprintln!(
            "PF10 fallback listing kept {}: exit {code}, daemon alive {a}, a hidden log found by name {hidden}",
            n - 1
        );
        assert!(
            !(code == 0 && a),
            "keep {}: exit 0 with the daemon alive: {}",
            n - 1,
            log_tail(&d)
        );
        if hidden {
            assert_eq!(
                code,
                ATTENTION,
                "keep {}: a hidden log keeps the end unproven: {}",
                n - 1,
                log_tail(&d)
            );
        }
        if n == 1 {
            assert!(
                hidden,
                "an EMPTY listing hides the crashed session's log: found by name: {}",
                log_tail(&d)
            );
        }
    }
    assert!(seen_hidden);
}

/// The files a closing-loop pass writes under FIXED names: its registry sentinels (procfix7 round 20: the product's own
/// list, so that the test and the walk cannot drift apart).
const CLOSELOOP_FIXED: &[&str] = rustred_campaign::closeloop::pass::FINAL_SENTINELS;

/// G10-B1 for the closing loop's final-directory walk (site `closeloop.output`: which phase-D files enter the registry
/// that resume and `verify` re-check): every listing of the walk ends early WITHOUT an error after k entries. A file
/// under a fixed name that exists but was not listed fails the pass (typed, never closed with exit 0); when the run does
/// end 0 with the node closed, every such file is in the registry (`final_files_blake3`). Control: listings ending only
/// after 63 entries (none truncated) close the node with exit 0 and a complete registry.
#[test]
fn g10_b1_truncated_closeloop_output_listing_never_closes_with_an_incomplete_registry() {
    if !in_ns() {
        return;
    }
    use closeloop_support::*;
    let _g = serial();
    for n in [1usize, 2, 3, 4, 64] {
        let t = Tmp::new(&format!("pf10-cl-{n}"));
        let files = toy(&t.path.join("material"), &Toy::default(), FAKE);
        // the hand statement (comparison only), as closeloop_fake and gate10_sites write it before a run
        let hand: String = [
            ([5, 1, 0], true, "base", "A#0"),
            ([4, 1, 0], false, "recipe", "R1"),
            ([3, 1, 0], false, "recipe", "R1"),
            ([2, 1, 0], false, "recipe", "R1"),
            ([1, 1, 0], false, "recipe", "default-recipe"),
            ([3, 2, 0], true, "recipe", "R2"),
            ([2, 2, 0], false, "recipe", "R2"),
            ([1, 2, 0], false, "recipe", "R2"),
        ]
        .iter()
        .map(|(k, b, l, r)| {
            format!(
                "{}\n",
                serde_json::json!([format!("I({},{},{})", k[0], k[1], k[2]), b, l, r])
            )
        })
        .collect();
        std::fs::write(&files.hand, hand).unwrap();
        let spec = campaign_spec(&t.campaign, &files, FAKE, &cpus());
        let sp = write_spec(&t, "spec.json", &spec);
        let d = t.cdir("cdir");
        let fp = format!(
            "fsx_trunc_site:{},fsx_trunc_at:{n}",
            site_index("closeloop.output")
        );
        let code = run(
            &sp,
            &d,
            &[("RR_CAMPAIGN_FAILPOINT", &fp)],
            Duration::from_secs(900),
        );
        let st = state(&d);
        let closed = st
            .nodes
            .get("toy")
            .is_some_and(|n| n.state.phase == rustred_campaign::state::NodePhase::Closed);
        let fin = d.join("closeloop/toy/final");
        let result: Option<serde_json::Value> = std::fs::read(fin.join("result.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok());
        let registry: Vec<String> = result
            .as_ref()
            .and_then(|r| r["final_files_blake3"].as_object())
            .map(|o| o.keys().cloned().collect())
            .unwrap_or_default();
        let missing: Vec<&str> = CLOSELOOP_FIXED
            .iter()
            .copied()
            .filter(|rel| fin.join(rel).is_file() && !registry.iter().any(|k| k == rel))
            .collect();
        // a failed pass: its result.json (verdict error, "error") and the ledger note "closeloop toy: pass N failed: .."
        let by_sentinel = result
            .as_ref()
            .and_then(|r| r["error"].as_str())
            .is_some_and(|e| e.contains("(an incomplete listing)"))
            || notes(&d, "closeloop toy: pass")
                .iter()
                .any(|n| n.contains("(an incomplete listing)"));
        eprintln!(
            "PF10 closeloop.output listings kept {}: exit {code}, toy closed {closed}, registry {} file(s), fixed-name files missing from it {missing:?}, failed by a sentinel {by_sentinel}",
            n - 1,
            registry.len()
        );
        if n == 64 {
            let derived: Vec<&String> = registry
                .iter()
                .filter(|k| !CLOSELOOP_FIXED.contains(&k.as_str()))
                .collect();
            eprintln!(
                "PF10 closeloop.output control: registered files under data-derived names (no sentinel): {derived:?}"
            );
            assert!(
                derived.is_empty(),
                "every registered file of the toy has a sentinel: {derived:?}"
            );
            assert!(
                code == 0 && closed && missing.is_empty() && !registry.is_empty(),
                "control (nothing truncated): {}",
                log_tail(&d)
            );
            continue;
        }
        if !(code == 0 && closed) {
            assert!(
                by_sentinel,
                "keep {}: the pass failed, but not by its listing sentinels: {:?} {:?}",
                n - 1,
                result.as_ref().map(|r| r["error"].clone()),
                notes(&d, "closeloop")
            );
        }
        if code == 0 && closed {
            assert!(
                missing.is_empty(),
                "keep {}: closed with exit 0 and the registry lacks {missing:?}: {}",
                n - 1,
                log_tail(&d)
            );
        }
        if n == 1 {
            assert!(
                !(code == 0 && closed),
                "an EMPTY walk never closes the node: {}",
                log_tail(&d)
            );
        }
    }
}

/// gate10 m10-2 (signal storm): a crashed session's marked daemon whose /proc entries are persistently unreadable (every
/// environ read and every 2nd stat read fail: test-build procfs faults on its pid) keeps every sweep and every job-end
/// hunt of the resumed session inconclusive for the whole unproven window (60 s here). The ledger gets records only for
/// STATE CHANGES: the group SIGKILL of a pinned group once, re-hunts backed off (5 s doubling): a bounded number of
/// `signal_sent` records, never one per round (gate10 measured ~160/s); and the end stays typed (exit 8).
#[test]
fn g10_m10_2_a_persistent_unreadable_process_never_floods_the_ledger() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("pf10-m10-2");
    let (sp, d, dp) = crashed_with_daemon_lim(&t, Some(60));
    let before = records(&d).len();
    let fp = format!("proc_environ_eio:1,proc_stat_eio:2,proc_pid:{dp}");
    let t0 = Instant::now();
    let code = run(
        &sp,
        &d,
        &[("RR_CAMPAIGN_FAILPOINT", &fp)],
        Duration::from_secs(900),
    );
    let took = t0.elapsed();
    let recs = records(&d);
    let new = &recs[before..];
    let signals = new
        .iter()
        .filter(|r| matches!(r.event, rustred_campaign::ledger::Event::SignalSent { .. }))
        .count();
    let ledger_bytes: u64 = std::fs::read_dir(rustred_campaign::ledger::ledger_dir(&d))
        .unwrap()
        .map(|e| e.unwrap().metadata().unwrap().len())
        .sum();
    let a = alive(dp);
    if a {
        kill_pid(dp);
    }
    eprintln!(
        "PF10 m10-2: exit {code} after {took:?}, {} new records, {signals} signal_sent, ledger {ledger_bytes} bytes, daemon alive {a}",
        new.len()
    );
    assert_eq!(code, ATTENTION, "{}", log_tail(&d));
    assert!(
        signals <= 40,
        "{signals} signal_sent records in {took:?}: one per round (a storm): {}",
        log_tail(&d)
    );
    assert!(
        new.len() <= 400,
        "{} records in {took:?}: {}",
        new.len(),
        log_tail(&d)
    );
}

/// Builds one of gate10's C helpers (scripts/gate10/src) with cc into `dir`; None without a C compiler.
fn build_gate10_c(dir: &Path, src: &str, out: &str, shared: bool) -> Option<PathBuf> {
    let srcp = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../scripts/gate10/src")
        .join(src);
    let o = dir.join(out);
    let mut c = Command::new("cc");
    if shared {
        c.args(["-shared", "-fPIC"]);
    }
    c.args(["-O1", "-o"]).arg(&o).arg(&srcp);
    if shared {
        c.args(["-ldl", "-lpthread"]);
    }
    let ok = c
        .env("PATH", path_env())
        .status()
        .is_ok_and(|s| s.success());
    ok.then_some(o)
}

/// One batch_rv run (gate10 scripts/gate10/batch_rv.sh + g10_chain_inner.sh): a crashed session's marked setsid'd
/// fork chain (g10chain.c) at `hop1_us` per hop; the resumed session runs with every pidfd_open delayed `delay_us`
/// (g10fault.c: a descheduled supervisor between the marker read and the pidfd); once its recovery sweep line is logged
/// the chain switches to `hop2_us`. (exit code, chain alive after, the recovery sweep recorded a vanish)
fn batch_rv_once(
    tag: &str,
    chain: &Path,
    fault: &Path,
    hop1_us: u64,
    hop2_us: u64,
    delay_us: u64,
) -> (i32, bool, bool) {
    let t = Tmp::new(tag);
    let ch = Fifo::new(&t.path);
    let trig = t.path.join("trigger");
    let sp = spec_with(
        &t,
        &format!(
            "{} {} {hop1_us} {} {hop2_us}",
            chain.display(),
            ch.path.display(),
            trig.display()
        ),
        [1500, 2500],
    );
    let d = t.cdir("c");
    let mut s1 = start(&sp, &d, &[], &[]);
    wait_until(
        "the chain",
        Duration::from_secs(300),
        || matches!(ch.read(), Ok(n) if n > 0),
    );
    std::thread::sleep(Duration::from_millis(300));
    signal(&s1, libc::SIGKILL);
    let _ = s1.wait();
    assert!(ch.alive(), "the chain runs after the crash");
    let so = fault.to_str().unwrap().to_string();
    let delay = delay_us.to_string();
    // session 1 wrote its own recovery line into the same log: only what session 2 writes counts (gate10's harness kept
    // run1.out and run2.out apart)
    let log = d.with_extension("log");
    let base = std::fs::read(&log).map(|b| b.len()).unwrap_or(0);
    let mut s2 = start(
        &sp,
        &d,
        &[],
        &[
            ("LD_PRELOAD", &so),
            ("G8_MATCH", "*"),
            ("G8_OPS", "pidfd"),
            ("G8_ERRNO", "delay"),
            ("G8_DELAY_US", &delay),
            ("G8_FROM", "1"),
        ],
    );
    let t0 = Instant::now();
    while !std::fs::read(&log)
        .map(|b| {
            String::from_utf8_lossy(&b[base.min(b.len())..]).contains("recovery: marker sweep")
        })
        .unwrap_or(false)
        && t0.elapsed() < Duration::from_secs(300)
    {
        std::thread::sleep(Duration::from_millis(20));
    }
    std::fs::write(&trig, b"").unwrap();
    let code = wait(&mut s2, Duration::from_secs(900));
    let alive = ch.alive_settled();
    let rec_vanished = notes(&d, "sweep_vanished {")
        .iter()
        .any(|n| n.contains("recovery sweep"));
    eprintln!(
        "PF10 {tag}: hop {hop1_us} -> {hop2_us} us, pidfd delay {delay_us} us: exit {code}, chain alive {alive}, recovery saw a vanish {rec_vanished}"
    );
    for l in std::fs::read_to_string(&log).unwrap_or_default().lines() {
        if l.contains("marker sweep")
            || l.contains("vanished")
            || l.contains("were seen alive")
            || l.contains("token")
            || l.contains("lineage")
        {
            eprintln!("PF10 {tag} log: {}", &l[..l.len().min(300)]);
        }
    }
    if rec_vanished {
        assert_ne!(
            code,
            0,
            "{tag}: exit 0 after the recovery sweep recorded vanished marked processes: {}",
            log_tail(&d)
        );
        assert!(notes(&d, "sweep_vanished_resolved").is_empty());
    }
    drop(ch);
    (code, alive, rec_vanished)
}

/// gate10's batch_rv.sh as a test (M10-1: the evidence of a chain must never be dropped). The recovery sweep of the
/// resumed session sees incarnations of the crashed session's chain alive that are gone before their stop (vanished);
/// right after it the chain runs with no pause (invisible to every later scan). Before the fix 3 of 6 runs ended 0 with
/// the chain alive after recovery logged vanished pids. Now a vanish is resolved only by an ATOMIC proof (none exists
/// for a crashed session's chain in a namespace): never exit 0 after a recorded vanish.
/// (A) gate10's parameters (5 ms per hop, pidfd_open delayed 30 ms): on a loaded host the 5 ms chain may be seen by NO
///     sweep at all (then exit 0 with it alive is the documented residual of a namespace run without a job scope,
///     RUNBOOK 2.7; reported, not asserted).
/// (B) the same scenario made load-proof (200 ms per hop, pidfd_open delayed 300 ms > one hop: every incarnation a
///     scan reads is gone before its pidfd): the evidence must appear (at least 2 of 3 runs), and never exit 0 after it.
#[test]
fn g10_batch_rv_recovery_vanished_evidence_is_never_dropped() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let _r = OrphanReaper::start("g10chain");
    let bins = Tmp::new("pf10-rv-bin");
    let (Some(chain), Some(fault)) = (
        build_gate10_c(&bins.path, "g10chain.c", "g10chain", false),
        build_gate10_c(&bins.path, "g10fault.c", "g10fault.so", true),
    ) else {
        eprintln!("SKIPPED: no C compiler for gate10's helpers");
        return;
    };
    let mut residual = 0;
    for i in 0..3 {
        let (code, alive, ev) =
            batch_rv_once(&format!("pf10-rvA-{i}"), &chain, &fault, 5000, 0, 30_000);
        if code == 0 && alive && !ev {
            residual += 1;
        }
    }
    let mut with_evidence = 0;
    for i in 0..3 {
        let (code, alive, ev) = batch_rv_once(
            &format!("pf10-rvB-{i}"),
            &chain,
            &fault,
            200_000,
            0,
            300_000,
        );
        with_evidence += usize::from(ev);
        assert!(
            !(code == 0 && alive && ev),
            "B {i}: exit 0 with the chain alive after recorded evidence"
        );
    }
    eprintln!(
        "PF10 batch_rv: (A) runs with no sweep seeing the chain and exit 0 alive (residual): {residual}/3; (B) runs with recovery evidence: {with_evidence}/3"
    );
    assert!(
        with_evidence >= 2,
        "(B) the load-proof scenario must produce recovery vanishes (got {with_evidence} of 3)"
    );
}
