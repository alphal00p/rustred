//! Lane procfix7 round 5 (gate9, WS/notes/implementer.gate9.md): the two BLOCKERS of gate 9 on 9876cba2 and their classes.
//!
//! G9-B1 (the filesystem facts rule, src/fsx.rs): a STAT error of ledger/MANIFEST.json was "no ledger": the residue of the
//! crashed session's ledger was deleted, a new ledger created, exit 0 with the live marked daemon. After: a manifest that
//! cannot be checked is a typed stop (70 / 71) with nothing opened, created or deleted; a non-empty ledger directory
//! without a manifest is refused, never repaired by deletion.
//! - `g9_b1_*`: the in-process fault layer at the manifest's site (every check of it fails, EIO and EMFILE);
//! - `g9_fault_property_*`: a crashed session with a live marked daemon; session 2 under ONE injected fault at helper call
//!   n (n = 1, 2, ...) or under EVERY call from n on: never exit 0 with the daemon alive, and session 1's ledger records
//!   are never altered (the later ledger starts with exactly them).
//!
//! G9-B2 (a non-atomic observation taken as a proof of absence): a marked process whose main thread exited (zombie
//! leader) and whose threads replace themselves was "a zombie without a live thread" for every scan. After: "no live
//! thread" only by the kernel's atomic count (`Threads:` == 1 with the leader a zombie); the final sweep has the atomic
//! own-tree proof (waitid ECHILD as a child subreaper) and never claims a clean end after a marked process vanished
//! before its stop. The churners are `rr_fake_child churn-threads | churn-procs` (test doubles), whose liveness the test
//! reads ATOMICALLY: they hold the write end of a FIFO; EOF on the test's end = every thread / incarnation is gone.
//! - thread churn (gate9's g9churn semantics), crashed and live sessions, life 50 us and 0: never exit 0 alive;
//! - process (fork) churn: in a live session (the own-tree proof) and crashed with 200 ms per hop: never exit 0 alive;
//!   crashed with 2 ms and no pause per hop: an ignored PROBE that reports what happens (the documented limit of a /proc
//!   scan without a job scope: RUNBOOK 2.7);
//! - ignored (needs a C compiler): gate9's own churner `g9churn.c` (copied verbatim), crashed, life 50 us and 0.
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

const INTERNAL: i32 = 70;
const RESOURCES: i32 = 71;

fn in_ns() -> bool {
    if rustred_campaign::launch::in_private_namespace() {
        orphan_reaper();
        return true;
    }
    eprintln!(
        "SKIPPED: spawns and signals processes: only inside a private pid namespace (rule L10)"
    );
    false
}

/// procfix7 round 7 (suite r11 on 2dfe705e: `g9_b2_fork_churner_crashed_session_200ms` ended 0 with the chain alive).
/// Test code, ONLY while this test binary is pid 1 of its private namespace (LIGHT_PIDNS): ONE reaper thread for the
/// whole binary reaps ZOMBIE orphans re-parented to it whose command is `rr_fake_child` (the incarnations of the fork
/// chains; the same rule as `OrphanReaper` in tests/procfix10.rs). An init that never reaps them keeps them: the 0-us
/// chain of `g9_b2_fork_churner_live_session` left 40,741 zombies in the namespace, every later strict scan read all of
/// them (walk + listing 3.5-4.8 s, the reads 1.5-1.9 s, oldest pid first), longer than the 200 ms hop of the crashed
/// case, whose current incarnation had then exited before its turn in every scan. Reaps nothing else (the supervisors
/// and shells the tests spawn have other commands); nothing is ever signalled.
fn orphan_reaper() {
    static STARTED: std::sync::Once = std::sync::Once::new();
    if std::process::id() != 1 {
        return;
    }
    STARTED.call_once(|| {
        std::thread::spawn(|| {
            loop {
                fake_zombie_orphans(true);
                std::thread::sleep(Duration::from_millis(20));
            }
        });
    });
}

/// The zombie `rr_fake_child` orphans of this test binary (pid 1 of the namespace), each reaped when `reap`.
fn fake_zombie_orphans(reap: bool) -> usize {
    let mut n = 0;
    let Ok(rd) = std::fs::read_dir("/proc") else {
        return 0;
    };
    for e in rd.flatten() {
        let Some(pid) = e.file_name().to_str().and_then(|n| n.parse::<i32>().ok()) else {
            continue;
        };
        let Ok(st) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
            continue;
        };
        let (Some(a), Some(b)) = (st.find('('), st.rfind(')')) else {
            continue;
        };
        let rest: Vec<&str> = st[b + 2..].split_whitespace().collect();
        if &st[a + 1..b] == "rr_fake_child"
            && rest.first() == Some(&"Z")
            && rest.get(1) == Some(&"1")
        {
            n += 1;
            if reap {
                // SAFETY: waitpid on one exact pid with a null status pointer
                unsafe { libc::waitpid(pid, std::ptr::null_mut(), libc::WNOHANG) };
            }
        }
    }
    n
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

/// The test's end of a churner's FIFO: `alive()` is the kernel's answer (a writer still holds the FIFO open).
struct Churn {
    fd: OwnedFd,
    path: PathBuf,
}

impl Churn {
    fn new(dir: &Path) -> Churn {
        let path = dir.join("churn.fifo");
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
        Churn { fd, path }
    }

    /// Ok(n bytes) read, Ok(0) EOF, Err(EAGAIN) open writer without data.
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

    /// Wait until the churner has started (its first byte).
    fn started(&self, limit: Duration) {
        let t0 = Instant::now();
        loop {
            if matches!(self.read(), Ok(n) if n > 0) {
                return;
            }
            assert!(t0.elapsed() < limit, "the churner did not start");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// Is any thread / incarnation alive? EOF (read 0) = no writer holds the FIFO any more and nothing is left in it.
    /// The drain is BOUNDED: a writer that keeps the FIFO non-empty for 256 reads is alive (it wrote just now).
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

    /// Alive for a while after the run (an exit in flight gets 2 s)?
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

/// A spec whose first job starts `cmd` (through a wrapper of the fake child) once.
fn spec_with(t: &Tmp, cmd: &str) -> PathBuf {
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
        serde_json::json!({"items": 3, "nodes": 1, "rungs": 1, "seed": 5, "case_ms": [800, 1200],
            "p_crash": 0.0, "p_mem": 0.0, "p_norule": 0.0, "deps": 0.0}),
        serde_json::json!({"max_jobs": 2, "unproven_end_limit_s": 20}),
    );
    v["programs"]["rr_fake_child"]["path"] = serde_json::json!(wrap);
    write_spec(t, "spec.json", &v)
}

/// Session 1 SIGKILLed once `ready` holds (the crashed session).
fn crash_session_one(sp: &Path, d: &Path, ready: impl FnMut() -> bool) {
    let mut s1 = start(sp, d, &[], &[]);
    wait_until("session 1 ready", Duration::from_secs(90), ready);
    std::thread::sleep(Duration::from_millis(300));
    signal(&s1, libc::SIGKILL);
    let _ = s1.wait();
}

fn run(sp: &Path, d: &Path, env: &[(&str, &str)], limit: Duration) -> i32 {
    let mut c: Child = start(sp, d, &[], env);
    wait(&mut c, limit)
}

/// The churner verdict: never exit 0 while the churner lives.
fn churn_case(mode: &str, sub: &str, life: u64, extra: &str) -> (i32, bool) {
    let _g = serial();
    // precondition (procfix7 round 7): no zombie orphan of an earlier case is left (the reaper above takes them; a
    // namespace full of them made every scan longer than a 200 ms hop)
    if std::process::id() == 1 {
        let t0 = Instant::now();
        while fake_zombie_orphans(false) > 0 {
            assert!(
                t0.elapsed() < Duration::from_secs(60),
                "zombie orphans of an earlier case were not reaped"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    let t = Tmp::new(&format!("pf9-{sub}-{mode}-{life}"));
    let ch = Churn::new(&t.path);
    let sp = spec_with(
        &t,
        &format!(
            "{FAKE} {sub} {} {life} {extra} </dev/null >/dev/null 2>&1",
            ch.path.display()
        ),
    );
    let d = t.cdir("c");
    let code = if mode == "crash" {
        crash_session_one(&sp, &d, || matches!(ch.read(), Ok(n) if n > 0));
        assert!(ch.alive(), "the churner runs after the crash");
        run(&sp, &d, &[], Duration::from_secs(900))
    } else {
        let mut c = start(&sp, &d, &[], &[]);
        ch.started(Duration::from_secs(90));
        wait(&mut c, Duration::from_secs(900))
    };
    let alive = ch.alive_settled();
    eprintln!(
        "PF9 {sub} {mode} life {life}{extra}: exit {code}, churner alive {alive}; {}",
        log_tail(&d)
            .lines()
            .filter(|l| l.contains("sweep")
                || l.contains("not proven")
                || l.contains("FINISHED")
                || l.contains("descendant"))
            .take(6)
            .collect::<Vec<_>>()
            .join(" | ")
    );
    drop(ch); // the churner ends at its next write (EPIPE)
    (code, alive)
}

fn never_zero_alive(r: (i32, bool), what: &str) {
    assert!(
        !(r.0 == 0 && r.1),
        "{what}: exit 0 with the churner ALIVE (gate9 G9-B2)"
    );
}

/// G9-B2, gate9's case `ch-crash-50`: zombie leader, one thread at a time living 50 us; crashed session 1.
#[test]
fn g9_b2_thread_churner_crashed_session_life_50us() {
    if !in_ns() {
        return;
    }
    never_zero_alive(
        churn_case("crash", "churn-threads", 50, ""),
        "threads crash 50 us",
    );
}

/// G9-B2, gate9's case `ch-crash-0`: threads replacing themselves without a pause.
#[test]
fn g9_b2_thread_churner_crashed_session_life_0() {
    if !in_ns() {
        return;
    }
    never_zero_alive(
        churn_case("crash", "churn-threads", 0, ""),
        "threads crash 0",
    );
}

/// G9-B2, gate9's case `ch-live-50`: the same churner started by a job of the running session.
#[test]
fn g9_b2_thread_churner_live_session() {
    if !in_ns() {
        return;
    }
    never_zero_alive(
        churn_case("live", "churn-threads", 50, ""),
        "threads live 50 us",
    );
}

/// Control (gate9 `nozombie`): a live main thread: killed (exit 0 allowed then), never exit 0 alive.
#[test]
fn g9_b2_thread_churner_control_live_leader() {
    if !in_ns() {
        return;
    }
    never_zero_alive(
        churn_case("crash", "churn-threads", 0, "nozombie"),
        "threads crash 0 nozombie",
    );
}

/// Process churn in a LIVE session: every incarnation is a descendant of the supervisor (a child subreaper), so the
/// own-tree proof (waitid ECHILD) sees it whatever a scan misses: never exit 0 alive.
#[test]
fn g9_b2_fork_churner_live_session() {
    if !in_ns() {
        return;
    }
    never_zero_alive(churn_case("live", "churn-procs", 0, ""), "procs live 0");
}

/// Process churn after a crash, 200 ms per hop (longer than a scan of the namespace: every incarnation is read by some
/// scan, stopped before it forks, killed): never exit 0 alive.
#[test]
fn g9_b2_fork_churner_crashed_session_200ms() {
    if !in_ns() {
        return;
    }
    never_zero_alive(
        churn_case("crash", "churn-procs", 200_000, ""),
        "procs crash 200 ms",
    );
}

/// PROBE (documented limit, RUNBOOK 2.7): process churn after a crash, 2 ms and 0 per hop. The chain is not a
/// descendant of the new session, and a /proc scan misses an incarnation that forks its successor and exits between the
/// scan's listing and its read of that pid (the gap grows with the number of processes read before it: milliseconds in
/// a namespace, more on a busy host). Reports the outcome of each; it does not assert (an exit 0 with the chain alive is
/// the "no marked process seen in 2 consecutive stable scans" the final message states). Observed on this commit
/// (namespace): 2 ms per hop killed in one run, missed in another.
#[test]
#[ignore = "probe of the documented limit of a /proc scan without a job scope; run explicitly"]
fn g9_b2_fork_churner_crashed_session_fast_probe() {
    if !in_ns() {
        return;
    }
    for life in [2000u64, 0] {
        let (code, alive) = churn_case("crash", "churn-procs", life, "");
        eprintln!("PROBE procs crash {life} us: exit {code}, churner alive {alive}");
    }
}

/// gate9's own churner (g9churn.c, verbatim), crashed session, life 50 us and 0 (its liveness as gate9 reads it: the
/// pid's task count > 1).
#[test]
#[ignore = "builds gate9's g9churn.c with cc; run explicitly"]
fn g9_b2_gate9_churner_never_gives_exit_0() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/procfix9_g9churn.c");
    let probe = Tmp::new("pf9-g9cc");
    let bin = probe.path.join("g9churn");
    let built = Command::new("cc")
        .args(["-O1", "-pthread", "-o"])
        .arg(&bin)
        .arg(&src)
        .env("PATH", path_env())
        .status();
    if !built.is_ok_and(|s| s.success()) {
        eprintln!("SKIPPED: no C compiler for gate9's churner");
        return;
    }
    for life in [50u64, 0] {
        let t = Tmp::new(&format!("pf9-g9-{life}"));
        let pidf = t.path.join("churn.pid");
        let sp = spec_with(&t, &format!("{} {} {life}", bin.display(), pidf.display()));
        let d = t.cdir("c");
        crash_session_one(&sp, &d, || pidf.exists());
        let cp: u32 = std::fs::read_to_string(&pidf)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        // procfix7 round 6 (gate10 m10-7): the kernel's ATOMIC thread count (status Threads:), never the entries of one
        // readdir of /proc/<pid>/task (it misses the live thread of a churner: measured, see tests/gate9_churn.rs)
        let tasks = |p: u32| -> usize {
            std::fs::read_to_string(format!("/proc/{p}/status"))
                .ok()
                .and_then(|s| {
                    s.lines()
                        .find_map(|l| l.strip_prefix("Threads:"))
                        .and_then(|v| v.trim().parse().ok())
                })
                .unwrap_or(0)
        };
        assert!(tasks(cp) > 1, "the churner runs after the crash");
        let code = run(&sp, &d, &[], Duration::from_secs(900));
        std::thread::sleep(Duration::from_secs(1));
        let alive = tasks(cp) > 1;
        eprintln!("PF9 gate9 g9churn life {life}: exit {code}, alive {alive}");
        if alive {
            // clean-up: the churner is ours (this test's); SIGKILL to its exact pid (the shell's builtin)
            let _ = Command::new("sh")
                .args(["-c", &format!("kill -KILL {cp} 2>/dev/null")])
                .env("PATH", path_env())
                .status();
        }
        assert!(
            !(code == 0 && alive),
            "gate9 g9churn life {life}: exit 0 with the churner alive"
        );
    }
}

// ------------------------------------------------------------------------------------------------ G9-B1

/// A crashed session 1 whose first job left a MARKED daemon (setsid sleep, reparented to the namespace's init).
/// Returns (spec, cdir, daemon pid).
fn crashed_with_daemon(t: &Tmp) -> (PathBuf, PathBuf, u32) {
    let dpf = t.path.join("daemon.pid");
    let sp = spec_with(
        t,
        &format!(
            "( setsid sleep 600 </dev/null >/dev/null 2>&1 & echo $! > {p}.tmp; mv {p}.tmp {p} )",
            p = dpf.display()
        ),
    );
    let d = t.cdir("c");
    crash_session_one(&sp, &d, || dpf.exists());
    let dp: u32 = std::fs::read_to_string(&dpf)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(alive(dp), "the daemon runs after the crash");
    (sp, d, dp)
}

fn alive(pid: u32) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .and_then(|s| s.rfind(')').map(|i| !s[i + 2..].starts_with('Z')))
        .unwrap_or(false)
}

/// Every file of the ledger directory with its bytes (the "untouched" check).
fn ledger_files(d: &Path) -> Vec<(String, Vec<u8>)> {
    let ldir = rustred_campaign::ledger::ledger_dir(d);
    let mut v: Vec<(String, Vec<u8>)> = std::fs::read_dir(&ldir)
        .unwrap()
        .map(|e| {
            let e = e.unwrap();
            (
                e.file_name().to_string_lossy().into_owned(),
                std::fs::read(e.path()).unwrap_or_default(),
            )
        })
        .collect();
    v.sort();
    v
}

fn site_index(name: &str) -> usize {
    rustred_campaign::fsx::SITES
        .iter()
        .position(|s| *s == name)
        .unwrap()
        + 1
}

/// G9-B1 in-process: every check of ledger/MANIFEST.json fails (EIO, then EMFILE): session 2 stops typed (70 / 71),
/// the ledger directory is byte for byte untouched, nothing was signalled (the daemon lives); a clean session 3 then
/// recovers: the daemon killed, exit 0.
#[test]
fn g9_b1_a_manifest_that_cannot_be_checked_is_a_typed_stop_with_nothing_touched() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("pf9-b1");
    let (sp, d, dp) = crashed_with_daemon(&t);
    let before = ledger_files(&d);
    for (errno, want) in [(libc::EIO, INTERNAL), (libc::EMFILE, RESOURCES)] {
        let fp = format!(
            "fsx_site:{},fsx_errno:{errno}",
            site_index("ledger.manifest")
        );
        let code = run(
            &sp,
            &d,
            &[("RR_CAMPAIGN_FAILPOINT", &fp)],
            Duration::from_secs(300),
        );
        assert_eq!(code, want, "errno {errno}: {}", log_tail(&d));
        assert_eq!(
            ledger_files(&d),
            before,
            "errno {errno}: the ledger was touched"
        );
        assert!(alive(dp), "errno {errno}: nothing may be signalled");
    }
    let code = run(&sp, &d, &[], Duration::from_secs(600));
    assert_eq!(code, 0, "{}", log_tail(&d));
    assert!(!alive(dp), "the recovering session kills the daemon");
}

/// The number of fsx helper calls of a clean session (test builds: `fsx_count:1`).
fn helper_calls(log: &str) -> Option<u64> {
    log.lines()
        .rev()
        .find_map(|l| l.split("TEST BUILD fsx helper calls: ").nth(1))
        .and_then(|n| n.trim().parse().ok())
}

/// The marker of a live process (test code reads /proc directly).
fn marker_of(pid: u32) -> String {
    let env = std::fs::read(format!("/proc/{pid}/environ")).unwrap();
    let var = format!("{}=", rustred_campaign::resources::MARKER_VAR);
    env.split(|&b| b == 0)
        .find_map(|kv| kv.strip_prefix(var.as_bytes()))
        .map(|v| String::from_utf8_lossy(v).into_owned())
        .expect("the daemon carries the marker")
}

/// A new marked daemon with `marker` (double fork: re-parented to the namespace's init). Returns its pid.
fn plant_daemon(dir: &Path, marker: &str) -> u32 {
    let pf = dir.join("planted.pid");
    let _ = std::fs::remove_file(&pf);
    let st = Command::new("sh")
        .args([
            "-c",
            &format!(
                "( setsid sleep 600 </dev/null >/dev/null 2>&1 & echo $! > {p}.tmp; mv {p}.tmp {p} )",
                p = pf.display()
            ),
        ])
        .env("PATH", path_env())
        .env(rustred_campaign::resources::MARKER_VAR, marker)
        .status()
        .unwrap();
    assert!(st.success());
    std::fs::read_to_string(&pf)
        .unwrap()
        .trim()
        .parse()
        .unwrap()
}

/// Records of the ledger (seq, bytes of the event) for the prefix check.
fn record_keys(d: &Path) -> Vec<String> {
    records(d)
        .iter()
        .map(|r| serde_json::to_string(r).unwrap())
        .collect()
}

/// The property (gate9 item 5): ANY single fault at helper call n, and every call from n on, of session 2 on a crashed
/// campaign with a live marked daemon gives a typed non-zero end or the daemon killed, never exit 0 with it alive; and
/// the records of every earlier session are never altered (only appends). `ns` single faults, `froms` persistent ones.
fn fault_property(tag: &str, ns: &[u64], froms: &[u64]) {
    let cases: Vec<String> = ns
        .iter()
        .map(|n| format!("fsx_at:{n}"))
        .chain(froms.iter().map(|n| format!("fsx_from:{n}")))
        .collect();
    fault_property_cases(tag, cases);
}

fn fault_property_cases(tag: &str, cases: Vec<String>) {
    let _g = serial();
    let t = Tmp::new(&format!("pf9-prop-{tag}"));
    let (sp, d, dp) = crashed_with_daemon(&t);
    let marker = marker_of(dp);
    let mut daemon = dp;
    let mut fails = Vec::new();
    let mut tally: std::collections::BTreeMap<i32, usize> = Default::default();
    for fp in cases {
        if !alive(daemon) {
            daemon = plant_daemon(&t.path, &marker);
        }
        let before = record_keys(&d);
        let code = run(
            &sp,
            &d,
            &[("RR_CAMPAIGN_FAILPOINT", &fp)],
            Duration::from_secs(600),
        );
        *tally.entry(code).or_default() += 1;
        let after = record_keys(&d);
        if code == 0 && alive(daemon) {
            fails.push(format!(
                "{fp}: exit 0 with the marked daemon {daemon} alive"
            ));
        }
        if after.len() < before.len() || after[..before.len()] != before[..] {
            fails.push(format!(
                "{fp}: the earlier records were altered ({} before, {} after)",
                before.len(),
                after.len()
            ));
        }
    }
    eprintln!(
        "PF9 property {tag}: exit codes {tally:?}; {} failure(s)",
        fails.len()
    );
    for f in fails.iter().take(20) {
        eprintln!("  {f}");
    }
    if alive(daemon) {
        // clean-up through a clean session (the daemon is the campaign's: its sweep kills it)
        let _ = run(&sp, &d, &[], Duration::from_secs(600));
    }
    assert!(fails.is_empty(), "{fails:?}");
}

/// The property over the start path (helper calls 1..=40 single, persistent from 1, 3, 10, 30) on every run of the
/// suite; the whole range is the ignored variant below.
#[test]
fn g9_fault_property_start_path() {
    if !in_ns() {
        return;
    }
    let ns: Vec<u64> = (1..=40).collect();
    fault_property("start", &ns, &[1, 3, 10, 30]);
}

/// The property with EVERY call at ONE site label failing (fsx_site:k for every k of `fsx::SITES`, EIO): every
/// decision site the synthetic campaign reaches gets its injected error (the counts of a clean session say which:
/// printed first).
#[test]
fn g9_fault_property_every_site() {
    if !in_ns() {
        return;
    }
    {
        let _g = serial();
        let t = Tmp::new("pf9-sites");
        let (sp, d, _dp) = crashed_with_daemon(&t);
        let code = run(
            &sp,
            &d,
            &[("RR_CAMPAIGN_FAILPOINT", "fsx_count:1")],
            Duration::from_secs(600),
        );
        let log = std::fs::read_to_string(d.with_extension("log")).unwrap_or_default();
        eprintln!(
            "PF9 a clean session 2 (exit {code}): {} helper calls; {}",
            helper_calls(&log).unwrap_or(0),
            log.lines()
                .rev()
                .find(|l| l.contains("TEST BUILD fsx sites:"))
                .unwrap_or("no site line")
        );
    }
    let cases: Vec<String> = (1..=rustred_campaign::fsx::SITES.len())
        .map(|k| format!("fsx_site:{k}"))
        .collect();
    fault_property_cases("sites", cases);
}

/// The property at EVERY helper call of a clean session 2 (counted first with `fsx_count:1`), single and persistent.
#[test]
#[ignore = "one session per helper call (hundreds); run explicitly"]
fn g9_fault_property_every_helper_call() {
    if !in_ns() {
        return;
    }
    let total = {
        let _g = serial();
        let t = Tmp::new("pf9-count");
        let (sp, d, _dp) = crashed_with_daemon(&t);
        let code = run(
            &sp,
            &d,
            &[("RR_CAMPAIGN_FAILPOINT", "fsx_count:1")],
            Duration::from_secs(600),
        );
        assert_eq!(code, 0, "{}", log_tail(&d));
        let log = std::fs::read_to_string(d.with_extension("log")).unwrap_or_default();
        helper_calls(&log).expect("the count line")
    };
    eprintln!("PF9 a clean session 2 makes {total} fsx helper calls");
    let ns: Vec<u64> = (1..=total).collect();
    let froms: Vec<u64> = (1..=total).step_by(7).collect();
    fault_property("every", &ns, &froms);
}

/// gate9 G9-B1 with gate9's own LD_PRELOAD injector (`g9fault.c`, verbatim) and the settings of its batch_mf2.sh (stat /
/// readdir of CDIR/ledger failing at candidates 1, 2 and 5 / 6, or stat at 1, 2; EIO, and ENOMEM): never exit 0 with the
/// crashed session's live marked daemon, and session 1's records are never altered (only appended).
#[test]
#[ignore = "builds gate9's LD_PRELOAD injector with cc; run explicitly"]
fn g9_b1_gate9_preload_injector_never_destroys_the_ledger() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/procfix9_g9fault.c");
    let probe = Tmp::new("pf9-g9fcc");
    let so = probe.path.join("g9fault.so");
    let built = Command::new("cc")
        .args(["-shared", "-fPIC", "-O1", "-o"])
        .arg(&so)
        .arg(&src)
        .arg("-ldl")
        .arg("-lpthread")
        .env("PATH", path_env())
        .status();
    if !built.is_ok_and(|s| s.success()) {
        eprintln!("SKIPPED: no C compiler for gate9's injector");
        return;
    }
    for (tag, ops, at, errno) in [
        ("mf2-r5", "stat,readdir", "1,2,5", "5"),
        ("mf2-r6", "stat,readdir", "1,2,6", "5"),
        ("mf2-r12", "stat", "1,2", "5"),
        ("mf2-r5m", "stat,readdir", "1,2,5", "12"),
    ] {
        let t = Tmp::new(&format!("pf9-{tag}"));
        let (sp, d, dp) = crashed_with_daemon(&t);
        let before = record_keys(&d);
        let flog = t.path.join("fault.log");
        let m = format!("{}/ledger", d.display());
        let env = [
            ("LD_PRELOAD", so.to_str().unwrap()),
            ("G8_LOG", flog.to_str().unwrap()),
            ("G8_MATCH", m.as_str()),
            ("G8_OPS", ops),
            ("G8_AT", at),
            ("G8_ERRNO", errno),
        ];
        let code = run(&sp, &d, &env, Duration::from_secs(600));
        let after = record_keys(&d);
        let injected = std::fs::read_to_string(&flog)
            .unwrap_or_default()
            .lines()
            .filter(|l| l.contains("FAILED"))
            .count();
        eprintln!(
            "PF9 gate9 {tag}: exit {code}, daemon alive {}, injected failures {injected}, records {} -> {}",
            alive(dp),
            before.len(),
            after.len()
        );
        assert!(
            !(code == 0 && alive(dp)),
            "{tag}: exit 0 with the marked daemon alive: {}",
            log_tail(&d)
        );
        assert!(
            after.len() >= before.len() && after[..before.len()] == before[..],
            "{tag}: session 1's records were altered"
        );
        if alive(dp) {
            let _ = run(&sp, &d, &[], Duration::from_secs(600));
        }
    }
}

/// gate9 minor (a mutation not caught: "drop the sweep's re-check of earlier suspects"). The crashed session's marked
/// daemon is a SUSPECT (its environ unreadable: procfs fault EIO on every environ read of its pid), and every later scan
/// of a sweep MISSES it (test-build hook `sweep_scan_drops_suspects:1`: a scan omits processes that are already
/// suspects). Only the direct re-check of earlier suspects still sees it: the sweeps stay unproven and session 2 ends
/// with exit 8 naming the daemon; without the re-check the second scan is "conclusive" and the run ends 0 with it alive.
#[test]
fn g9_minor_a_suspect_missed_by_later_scans_is_still_rechecked() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("pf9-suspect");
    let (sp, d, dp) = crashed_with_daemon(&t);
    let fp = format!("proc_environ_eio:1,proc_pid:{dp},sweep_scan_drops_suspects:1");
    let code = run(
        &sp,
        &d,
        &[("RR_CAMPAIGN_FAILPOINT", &fp)],
        Duration::from_secs(900),
    );
    let alive_after = alive(dp);
    eprintln!("PF9 suspect recheck: exit {code}, daemon alive {alive_after}");
    assert!(alive_after, "an unreadable daemon is never signalled");
    assert_eq!(code, ATTENTION, "{}", log_tail(&d));
    let log = std::fs::read_to_string(d.with_extension("log")).unwrap_or_default();
    assert!(
        log.lines()
            .any(|l| l.contains("not proven") && l.contains(&format!("{dp}"))),
        "the exit-8 message names the daemon {dp}: {}",
        log_tail(&d)
    );
    // clean-up through a clean session (its sweep kills the campaign's daemon)
    let _ = run(&sp, &d, &[], Duration::from_secs(600));
}

/// gate9 minor m9-clock: a panic of the control thread (std's Instant / SystemTime panic when clock_gettime fails) ended
/// with exit 101. Now a typed end: exit 70 with the reason on stderr and in the campaign log (test-build failpoint
/// `test_panic:1` panics where the supervisor run starts).
#[test]
fn g9_minor_a_panic_of_the_supervisor_is_a_typed_end() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("pf9-panic");
    let sp = spec_with(&t, "true");
    let d = t.cdir("c");
    let code = run(
        &sp,
        &d,
        &[("RR_CAMPAIGN_FAILPOINT", "test_panic:1")],
        Duration::from_secs(120),
    );
    assert_eq!(code, INTERNAL, "{}", log_tail(&d));
    let err = std::fs::read_to_string(d.with_extension("log")).unwrap_or_default();
    assert!(
        err.contains("the supervisor panicked") && err.contains("test_panic"),
        "the typed end names the panic: {err}"
    );
}
