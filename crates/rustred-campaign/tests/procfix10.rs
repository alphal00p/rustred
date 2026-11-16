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
    wait_until("the daemon", Duration::from_secs(90), || dpf.exists());
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
#[ignore = "every k (about 25 campaigns of up to 3 min each); run explicitly"]
fn g10_b1_truncated_root_listing_crashed_session_every_k() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    every_k("truncation crashed (every k)", &ks(true), trunc_crashed);
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
        let fd = unsafe { libc::open(c.as_ptr(), libc::O_RDONLY | libc::O_NONBLOCK) };
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
    every_k("truncation live", &ks(false), trunc_live);
}

#[test]
#[ignore = "every k (about 25 campaigns of up to 3 min each); run explicitly"]
fn g10_b1_truncated_root_listing_live_session_every_k() {
    if !in_ns() {
        return;
    }
    let _g = serial();
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
        Duration::from_secs(90),
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
    assert!(
        !(code == 0 && alive),
        "exit 0 with the chain alive: {}",
        log_tail(&d)
    );
    if code == 0 && !vanished.is_empty() {
        assert!(
            !resolved.is_empty(),
            "an exit 0 after a vanish needs a later proven seeded sweep: {}",
            log_tail(&d)
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
}

/// A crashed session whose first job left a marked daemon (setsid sleep): (spec, cdir, daemon pid).
fn crashed_with_daemon(t: &Tmp) -> (PathBuf, PathBuf, u32) {
    let dpf = t.path.join("daemon.pid");
    let sp = spec_with(
        t,
        &format!(
            "( setsid sleep 600 </dev/null >/dev/null 2>&1 & echo $! > {p}.tmp; mv {p}.tmp {p} )",
            p = dpf.display()
        ),
        [800, 1200],
    );
    let d = t.cdir("c");
    let mut s1 = start(&sp, &d, &[], &[]);
    wait_until("the daemon", Duration::from_secs(90), || dpf.exists());
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
    wait_until("the daemon", Duration::from_secs(90), || dpf.exists());
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
