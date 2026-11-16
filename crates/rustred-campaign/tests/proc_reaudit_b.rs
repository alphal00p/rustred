//! PORT of re-audit B's black-box harness (notes/implementer.reaudit-proc-B.md; original under
//! /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/reaudit-proc-B/rb, binary `rb`).
//!
//! Every scenario runs as pid 1 of its OWN FRESH private user + pid namespace: the scenario test (itself inside the
//! LIGHT_PIDNS namespace of this binary, rule L10) starts `unshare -Urpf --mount-proc --kill-child=SIGKILL -- <this test
//! binary> ... RBARG=ns RBARG=<scenario> RBARG=<run dir>`; inside, the DRIVER (pid 1) logs every signal it receives,
//! spawns and reaps every decoy, and runs the supervisor under test as a grandchild in its own session (so only the
//! layer's own rules protect pid 1). Roles of the original binary (`rb ns|mid|sup|helper|prop`) are reached through the
//! single test `rb_entry` with the role's arguments as trailing libtest filters `RBARG=<arg>` (libtest runs `rb_entry`
//! and ignores the extra filters; [`rb_cmd`] builds such a command line).
//!
//! Run (each scenario in its own namespace):
//! `LIGHT_PIDNS=1 WS/plan/auto-campaign/implementer_ops/light_build.sh procfix2 test -p rustred-campaign --test proc_reaudit_b`
//! and the long ones (churn 10,000 cycles, relay, vrace; release build):
//! `LIGHT_PIDNS=1 ... test -p rustred-campaign --release --test proc_reaudit_b -- --ignored --test-threads=1`
//! Environment: RB_CYCLES, RB_SEED, RB_PIDMAX=none, RB_NOFORCE=1, RB_FDSTORM=1 (as in the original); RB_LOG_DIR (default
//! /common/build-cache/$USER/p_integral_dev/implementer-scratch/procfix2/reaudit-b when it exists, else the temp dir).
//!
//! Each scenario passes when its output has no `| FAIL` result line, the driver's SUMMARY shows decoys_dead=0,
//! marked_orphans_missed=0 and driver_signals=none, and no churn decoy was killed by a signal.
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

#[path = "proc_reaudit_b/driver.rs"]
mod driver;
#[path = "proc_reaudit_b/helper.rs"]
mod helper;
#[path = "proc_reaudit_b/prop.rs"]
mod prop;
#[path = "proc_reaudit_b/sup.rs"]
mod sup;

use rustred_campaign::resources::{ProcStat, read_stat};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub const CAMP: &str = "rbcamp";

pub fn stat(pid: u32) -> Option<ProcStat> {
    read_stat(pid).ok()
}

/// Live (not zombie / dead) process with this start time? (A dead main thread with a live thread counts as alive.)
pub fn alive(pid: u32, start: u64) -> bool {
    stat(pid).is_some_and(|s| {
        s.start_ticks == start
            && (!s.is_dead()
                || matches!(
                    rustred_campaign::resources::live_thread(pid),
                    Ok(rustred_campaign::resources::LiveThread::Found(..))
                ))
    })
}

pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub fn unhex(s: &str) -> Vec<u8> {
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap())
        .collect()
}

pub fn which(name: &str) -> PathBuf {
    for d in std::env::var("PATH").unwrap_or_default().split(':') {
        let p = Path::new(d).join(name);
        if p.is_file() {
            return p;
        }
    }
    panic!("{name} not on PATH")
}

pub fn exe() -> PathBuf {
    std::env::current_exe().unwrap()
}

/// libtest arguments that run `rb_entry` with these role arguments.
pub fn rb_args(args: &[String]) -> Vec<String> {
    let mut v: Vec<String> = [
        "--exact",
        "rb_entry",
        "--nocapture",
        "--test-threads=1",
        "-q",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    v.extend(args.iter().map(|a| format!("RBARG={a}")));
    v
}

/// A command that re-executes this test binary in the role given by `args` (the original `rb <args>`).
pub fn rb_cmd(args: &[String]) -> Command {
    let mut c = Command::new(exe());
    c.args(rb_args(args));
    c
}

pub fn wait_for(timeout: Duration, mut f: impl FnMut() -> bool) -> bool {
    let t0 = Instant::now();
    while t0.elapsed() < timeout {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    f()
}

pub fn read_u32_file(p: &Path, timeout: Duration) -> Option<u32> {
    let mut v = None;
    wait_for(timeout, || {
        v = std::fs::read_to_string(p)
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok());
        v.is_some()
    });
    v
}

pub fn write_atomic(p: &Path, text: &str) {
    let tmp = p.with_extension("tmp");
    std::fs::write(&tmp, text).unwrap();
    std::fs::rename(&tmp, p).unwrap();
}

pub fn utc() -> String {
    Command::new("date")
        .args(["-u", "+%FT%TZ"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

/// Zombie children of this process (pid, comm).
pub fn my_zombies() -> Vec<(u32, String)> {
    let me = std::process::id();
    let mut out = vec![];
    if let Ok(d) = std::fs::read_dir("/proc") {
        for e in d.flatten() {
            if let Some(pid) = e.file_name().to_str().and_then(|s| s.parse::<u32>().ok())
                && let Some(s) = stat(pid)
                && s.ppid == me
                && s.state == 'Z'
            {
                out.push((pid, s.comm.clone()));
            }
        }
    }
    out
}

/// The role entry (does nothing in a normal run: no RBARG filter on the command line).
#[test]
fn rb_entry() {
    let args: Vec<String> = std::env::args()
        .filter_map(|a| a.strip_prefix("RBARG=").map(str::to_string))
        .collect();
    let Some(mode) = args.first().cloned() else {
        return;
    };
    match mode.as_str() {
        "ns" => driver::main(&args[1..]),
        "mid" => driver::mid(&args[1..]),
        "sup" => sup::main(&args[1..]),
        "helper" => helper::main(&args[1..]),
        "prop" => prop::main(&args[1..]),
        m => panic!("unknown role {m}"),
    }
}

fn log_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("RB_LOG_DIR") {
        return PathBuf::from(d);
    }
    let user = std::env::var("USER").unwrap_or_else(|_| "vhirschi".into());
    let d = PathBuf::from(format!(
        "/common/build-cache/{user}/p_integral_dev/implementer-scratch/procfix2/reaudit-b"
    ));
    if d.parent().is_some_and(Path::exists) {
        let _ = std::fs::create_dir_all(&d);
        return d;
    }
    std::env::temp_dir()
}

/// Run one scenario in a fresh nested private namespace; returns its output (also written to the log directory).
fn run_scenario(scen: &str, extra_env: &[(&str, &str)], timeout: Duration) -> String {
    assert_eq!(
        std::process::id(),
        1,
        "run inside a private pid namespace (LIGHT_PIDNS=1 light_build.sh ...)"
    );
    // SUBAGENT_RULES rule 2 since 2026-11-14T12:05Z: only the implementer account's flag binds (WS/ops/PAUSE reflects the
    // coordinator's account alone)
    let pause = "/shared/vhirschi/t3code/vakint_dev/p_integral_dev/plan/auto-campaign/implementer_ops/PAUSE";
    assert!(!Path::new(pause).exists(), "PAUSE flag {pause}");
    // One scenario at a time (host load; churn timing).
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    // lane int5: a new TMPDIR-rooted directory unique on the host (never the pid: it is 1 in the namespace)
    let run = unique_temp_dir(&format!("rr-reaudit-b-{scen}"));
    // A private runtime directory per scenario: the host-wide campaign-id lock of "rbcamp" is then private to it.
    let xdg = run.join("xdg");
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new().mode(0o700).create(&xdg).unwrap();
    }
    let mut c = Command::new("unshare");
    c.args(["-Urpf", "--mount-proc", "--kill-child=SIGKILL", "--"])
        .arg(exe())
        .args(rb_args(&[
            "ns".into(),
            scen.into(),
            run.display().to_string(),
        ]))
        .env_remove(rustred_campaign::resources::MARKER_VAR)
        .env("XDG_RUNTIME_DIR", &xdg)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in extra_env {
        c.env(k, v);
    }
    let t0 = Instant::now();
    let mut ch = c.spawn().expect("unshare");
    let mut out = ch.stdout.take().unwrap();
    let mut err = ch.stderr.take().unwrap();
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = out.read_to_string(&mut s);
        s
    });
    let ereader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = err.read_to_string(&mut s);
        s
    });
    let mut timed_out = false;
    loop {
        match ch.try_wait() {
            Ok(Some(_)) | Err(_) => break,
            Ok(None) if t0.elapsed() > timeout => {
                // Our own direct child (the unshare process); the namespace dies with it (--kill-child).
                let _ = ch.kill();
                let _ = ch.wait();
                timed_out = true;
                break;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
        }
    }
    let text = reader.join().unwrap_or_default();
    let etext = ereader.join().unwrap_or_default();
    let env_tag: String = extra_env.iter().map(|(k, v)| format!("-{k}={v}")).collect();
    let log = log_dir().join(format!("{scen}{env_tag}.log"));
    let _ = std::fs::write(
        &log,
        format!(
            "=== {} scenario {scen} {extra_env:?} ({:?}, timed out: {timed_out})\n{text}\n=== stderr\n{etext}",
            utc(),
            t0.elapsed()
        ),
    );
    println!("scenario {scen}: {:?}, log {}", t0.elapsed(), log.display());
    assert!(
        !timed_out,
        "scenario {scen} timed out; log {}",
        log.display()
    );
    text
}

/// The gate's checks on a scenario's output.
fn check(scen: &str, text: &str) {
    let fails: Vec<&str> = text
        .lines()
        .filter(|l| l.starts_with("RESULT") && l.ends_with("| FAIL"))
        .collect();
    for l in text.lines().filter(|l| {
        l.starts_with("RESULT")
            || l.starts_with("SUMMARY")
            || l.starts_with("CHURN")
            || l.starts_with("VRACE")
            || l.starts_with("SUPSUMMARY")
    }) {
        println!("{l}");
    }
    assert!(fails.is_empty(), "{scen}: failed results: {fails:#?}");
    let summary = text
        .lines()
        .find(|l| l.starts_with("SUMMARY"))
        .unwrap_or_else(|| panic!("{scen}: no SUMMARY line"));
    for want in [
        " decoys_dead=0 ",
        " marked_orphans_missed=0 ",
        " driver_signals=none ",
    ] {
        assert!(summary.contains(want), "{scen}: {summary}");
    }
    assert!(
        !text.contains("CHURN-KILLED"),
        "{scen}: a churn decoy was killed by a signal"
    );
    assert!(
        text.lines().any(|l| l.starts_with("SUPSUMMARY")),
        "{scen}: the supervisor side did not finish"
    );
}

fn scenario(scen: &str, env: &[(&str, &str)], secs: u64) {
    let text = run_scenario(scen, env, Duration::from_secs(secs));
    check(scen, &text);
}

#[test]
fn rb_pid1() {
    scenario("pid1", &[("RR_CAMPAIGN_JOB", "rbcamp:L1")], 600);
}
#[test]
fn rb_pid0() {
    scenario("pid0", &[], 600);
}
#[test]
fn rb_bigpid() {
    scenario("bigpid", &[], 600);
}
#[test]
fn rb_reuse() {
    scenario("reuse", &[], 600);
}
#[test]
fn rb_leaderless() {
    scenario("leaderless", &[], 600);
}
#[test]
fn rb_nonleader() {
    scenario("nonleader", &[], 600);
}
#[test]
fn rb_stalegroup() {
    scenario("stalegroup", &[], 600);
}
#[test]
fn rb_daemon() {
    scenario("daemon", &[], 600);
}
#[test]
fn rb_escapee() {
    scenario("escapee", &[], 600);
}
#[test]
fn rb_recover() {
    scenario("recover", &[], 600);
}
#[test]
fn rb_cpurange() {
    scenario("cpurange", &[], 600);
}
#[test]
fn rb_cow() {
    scenario("cow", &[], 600);
}
#[test]
fn rb_exitterm() {
    scenario("exitterm", &[], 600);
}
#[test]
fn rb_leak() {
    scenario("leak", &[], 600);
}
#[test]
fn rb_marker() {
    scenario("marker", &[], 600);
}
#[test]
fn rb_two() {
    scenario("two", &[], 600);
}
#[test]
fn rb_lease() {
    scenario("lease", &[], 600);
}
#[test]
fn rb_fd1() {
    scenario("fd1", &[], 600);
}
#[test]
fn rb_fd2() {
    scenario("fd2", &[], 600);
}
#[test]
fn rb_fd3() {
    scenario("fd3", &[], 600);
}
#[test]
fn rb_zombies() {
    scenario("zombies", &[], 600);
}
/// A short churn (200 cycles, pid reuse forced) in the normal suite; the 10,000-cycle runs are ignored (below).
#[test]
fn rb_churn_short() {
    scenario("churn", &[("RB_CYCLES", "200"), ("RB_SEED", "7")], 900);
}
/// A short verify race (400 self-ending jobs, pid_max 4096) in the normal suite: catches a marked process judged
/// unmarked in the execve window (empty environ), see resources::children::read_environ_settled.
#[test]
fn rb_vrace_short() {
    scenario("churn_vrace", &[("RB_CYCLES", "400")], 900);
}
#[test]
#[ignore = "long: relay at 8 fork rates (run with --ignored)"]
fn rb_churn_relay() {
    scenario("churn_relay", &[], 1700);
}
#[test]
#[ignore = "long: 2 x 4,000 self-ending jobs (run with --ignored)"]
fn rb_churn_vrace() {
    scenario("churn_vrace", &[("RB_CYCLES", "4000")], 1700);
}
#[test]
#[ignore = "long: 4,000 self-ending jobs without pid_max change (run with --ignored)"]
fn rb_vrace_nowrap() {
    scenario("vrace_nowrap", &[("RB_CYCLES", "4000")], 1700);
}
/// ACCEPTANCE: the re-audit's 60,200 churn cycles (seeds 101, 202, 303, 101 again, 404 with pid reuse; 505 without;
/// plus 200), each run as its own scenario namespace. `RB_ACCEPT_SEEDS` overrides the list (e.g. "101" for one run).
#[test]
#[ignore = "long: acceptance churn 60,200 cycles (run with --ignored, release)"]
fn rb_churn_acceptance() {
    let seeds =
        std::env::var("RB_ACCEPT_SEEDS").unwrap_or_else(|_| "101,202,303,101,404,505".into());
    let cycles = std::env::var("RB_ACCEPT_CYCLES").unwrap_or_else(|_| "10000".into());
    for seed in seeds.split(',') {
        let mut env = vec![("RB_CYCLES", cycles.as_str()), ("RB_SEED", seed)];
        if seed == "505" {
            env.push(("RB_PIDMAX", "none"));
            env.push(("RB_NOFORCE", "1"));
        }
        scenario("churn", &env, 1700);
    }
    scenario("churn", &[("RB_CYCLES", "200"), ("RB_SEED", "606")], 900);
}
/// ACCEPTANCE under fd exhaustion: the churn with periodic descriptor-table exhaustion (RB_FDSTORM) while jobs end.
#[test]
#[ignore = "long: acceptance churn with fd exhaustion (run with --ignored, release)"]
fn rb_churn_fdstorm_acceptance() {
    let seeds = std::env::var("RB_FDSTORM_SEEDS").unwrap_or_else(|_| "707,808".into());
    let cycles = std::env::var("RB_ACCEPT_CYCLES").unwrap_or_else(|_| "10000".into());
    for seed in seeds.split(',') {
        scenario(
            "churn",
            &[
                ("RB_CYCLES", cycles.as_str()),
                ("RB_SEED", seed),
                ("RB_FDSTORM", "1"),
            ],
            1700,
        );
    }
}
/// Admission property run of the original (`rb prop 778899 200000`): pure, no process.
#[test]
fn rb_prop() {
    let out = Command::new(exe())
        .args(rb_args(&["prop".into(), "778899".into(), "200000".into()]))
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let _ = std::fs::write(log_dir().join("prop.log"), &text);
    for l in text.lines().filter(|l| l.starts_with("PROP")) {
        println!("{l}");
    }
    let p1 = text
        .lines()
        .find(|l| l.starts_with("PROP-P1 floor/truth"))
        .expect("P1 line");
    assert!(p1.contains("violations_in_documented_categories=0"), "{p1}");
    // RB m5: the categories "measurement from the future" and "newer than the host reading" no longer admit below
    // the floor (they are treated as stale).
    for bad in ["[FutureLow]", "[NewerThanHostGrew]"] {
        assert!(
            !text.contains(&format!("UNDOCUMENTED-CATEGORY violations {bad}")),
            "{bad} still violates: {text}"
        );
    }
}

/// A NEW directory `<TMPDIR>/<prefix>-<token>` unique on the host (lane int5, procfix3 item 0): never derived from the
/// pid alone (inside the private pid namespaces of rule L10 every test binary is pid 1, and two lanes may share a temp
/// root), and created with `create_dir` (a directory it did not create is never reused or removed).
#[allow(dead_code)]
pub fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let base = std::env::temp_dir();
    std::fs::create_dir_all(&base).unwrap();
    let ns = std::fs::read_link("/proc/self/ns/pid")
        .map(|l| l.to_string_lossy().into_owned())
        .unwrap_or_default();
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    loop {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let n = SEQ.fetch_add(1, Ordering::Relaxed);
        let token = blake3::hash(format!("{ns}|{}|{stat}|{t}|{n}", std::process::id()).as_bytes())
            .to_hex()[..12]
            .to_string();
        let p = base.join(format!("{prefix}-{token}"));
        match std::fs::create_dir(&p) {
            Ok(()) => return p,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => panic!("{}: {e}", p.display()),
        }
    }
}
