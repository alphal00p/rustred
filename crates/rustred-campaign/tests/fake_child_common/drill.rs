//! Harness of the drills ported from audit-contract-B (t1 kill drill, t4 classifier grid; lane contractfix): spawn
//! rr_fake_child (own session, hard timeout), build specs as JSON, classify with the supervisor's own spec digest.
//! Run directories live under `RR_DRILL_DIR` (default: the system temp dir).

use rustred_campaign_contract::classify::{
    Classification, Evidence, KillReason, WaitStatus, assess, classify,
};
use rustred_campaign_contract::layout::RunLayout;
use rustred_campaign_contract::spec::JobSpec;
use serde_json::{Value, json};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

pub const D0: &str = "0000000000000000000000000000000000000000000000000000000000000000";
static N: AtomicU64 = AtomicU64::new(0);

/// The drill's scratch root: `RR_DRILL_DIR` when set (kept for inspection), else ONE TMPDIR-rooted directory unique to
/// this test process (lane int5: never the pid alone).
pub fn scratch() -> PathBuf {
    static ROOT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    std::env::var_os("RR_DRILL_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| ROOT.get_or_init(|| super::unique_temp_dir("rr-drill")).clone())
}
pub fn bin() -> PathBuf {
    PathBuf::from(super::BIN)
}

/// A fresh directory under scratch/runs/<tag>-<token>-<n> (created new; never an existing one removed).
pub fn fresh(tag: &str) -> PathBuf {
    let runs = scratch().join("runs");
    std::fs::create_dir_all(&runs).unwrap();
    let u = super::unique_temp_dir(&format!("rr-drill-run-{tag}"));
    let _ = std::fs::remove_dir(&u);
    let p = runs.join(format!(
        "{}-{}",
        u.file_name().unwrap().to_string_lossy(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    p
}

/// Deterministic xorshift64*.
#[derive(Clone)]
pub struct Rng(pub u64);
impl Rng {
    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 { 0 } else { self.next() % n }
    }
    pub fn chance(&mut self, num: u64, den: u64) -> bool {
        self.below(den) < num
    }
}

/// Fake spec as JSON (what a supervisor writes).
pub fn spec_json(run_dir: &Path, job: &str, attempt: u32, fake: Value) -> Value {
    json!({
        "schema":"rustred.child-job.v1","job_id":job,"attempt":attempt,"kind":"fake",
        "run_dir": run_dir.to_str().unwrap(),
        "program":{"name":"rr_fake_child","blake3":D0,"features_required":["fake"]},
        "seed":{"sample_seed":7},
        "heartbeat_s":1,"stop_grace_s":2,"resume_every_s":0,
        "fake": fake
    })
}

pub fn write_spec(run_dir: &Path, spec: &Value) -> PathBuf {
    std::fs::create_dir_all(run_dir).unwrap();
    let p = run_dir.join("job.json");
    let _ = std::fs::remove_file(&p);
    std::fs::write(&p, serde_json::to_vec(spec).unwrap()).unwrap();
    p
}

pub fn parse_spec(run_dir: &Path) -> JobSpec {
    JobSpec::from_json(&std::fs::read(run_dir.join("job.json")).unwrap()).unwrap()
}

/// Spawn options.
#[derive(Default, Clone)]
pub struct SpawnOpt {
    /// RLIMIT_FSIZE in bytes (deterministic death at byte N of any file).
    pub fsize: Option<u64>,
    /// Ignore SIGXFSZ (then writes beyond the limit fail with EFBIG).
    pub ignore_xfsz: bool,
}

pub fn spawn(run_dir: &Path, opt: &SpawnOpt) -> Child {
    let mut c = Command::new(bin());
    c.arg("job").arg(run_dir.join("job.json"));
    c.stdin(Stdio::null())
        .stdout(Stdio::from(
            std::fs::File::create(run_dir.with_extension("stdout")).unwrap(),
        ))
        .stderr(Stdio::from(
            std::fs::File::create(run_dir.with_extension("stderr")).unwrap(),
        ));
    c.env_remove("RUSTRED_PROBE_POINT");
    let fsize = opt.fsize;
    let ign = opt.ignore_xfsz;
    // SAFETY: only async-signal-safe calls between fork and exec.
    unsafe {
        c.pre_exec(move || {
            libc::setsid();
            if let Some(n) = fsize {
                let r = libc::rlimit {
                    rlim_cur: n,
                    rlim_max: n,
                };
                libc::setrlimit(libc::RLIMIT_FSIZE, &r);
            }
            if ign {
                libc::signal(libc::SIGXFSZ, libc::SIG_IGN);
            }
            // hard CPU bound for safety
            let r = libc::rlimit {
                rlim_cur: 120,
                rlim_max: 120,
            };
            libc::setrlimit(libc::RLIMIT_CPU, &r);
            Ok(())
        });
    }
    c.spawn().expect("spawn rr_fake_child")
}

pub fn wait_status(st: std::process::ExitStatus) -> WaitStatus {
    match (st.code(), st.signal()) {
        (Some(c), _) => WaitStatus::Exited(c),
        (None, Some(s)) => WaitStatus::Signaled(s),
        _ => WaitStatus::Unknown,
    }
}

/// Wait with a hard timeout; on timeout SIGKILL the group and reap.
pub fn wait_timeout(ch: &mut Child, t: Duration) -> (WaitStatus, bool) {
    let t0 = Instant::now();
    loop {
        if let Some(st) = ch.try_wait().unwrap() {
            return (wait_status(st), false);
        }
        if t0.elapsed() > t {
            kill_group(ch.id() as i32, libc::SIGKILL);
            let st = ch.wait().unwrap();
            return (wait_status(st), true);
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Signal the child's own group (it is a session leader we created).
pub fn kill_group(pgid: i32, sig: i32) {
    // SAFETY: our own child's process group (setsid in pre_exec).
    unsafe {
        libc::kill(-pgid, sig);
    }
}

pub fn run_to_end(run_dir: &Path, opt: &SpawnOpt, t: Duration) -> WaitStatus {
    let mut ch = spawn(run_dir, opt);
    let (w, to) = wait_timeout(&mut ch, t);
    assert!(!to, "timeout in {}", run_dir.display());
    w
}

/// The supervisor's digest of the job.json it wrote.
pub fn spec_digest(run_dir: &Path) -> String {
    rustred_campaign_contract::blake3_hex(&std::fs::read(run_dir.join("job.json")).unwrap())
}

pub fn classify_dir(run_dir: &Path, w: WaitStatus, k: KillReason) -> (Evidence, Classification) {
    let spec = parse_spec(run_dir);
    let layout = RunLayout::new(run_dir);
    let e = assess(&layout, &spec, &spec_digest(run_dir), w, k, false);
    let c = classify(&e);
    (e, c)
}

/// Rule case op.
pub fn rule_case(ordinal: u64, ms: u64, loci: Option<(u32, u64)>) -> Value {
    let mut rule = json!({"record":{"chart":"11111","case":ordinal,"target":format!("I({ordinal},1,1)"),
        "rhs":[["I(0,1,1)","-1/2"],["I(1,0,1)",format!("{ordinal}")]]}});
    if let Some((n, len)) = loci {
        rule["loci"] = json!({"n":n,"len":len});
    }
    json!({"op":"case","ordinal":ordinal,"ms":ms,"work":{"seeds":10*(ordinal+1),"rows":100},"result":"rule",
        "rule":rule,"children":[{"frame":"PRESENTED","fixed":[ordinal,null],"equations":[]}]})
}
pub fn covered_case(ordinal: u64, ms: u64) -> Value {
    json!({"op":"case","ordinal":ordinal,"ms":ms,"work":{"seeds":3},"result":"covered"})
}

/// RESUME-derived resume ref for a new attempt (the classifier's advice), or None if RESUME is not valid.
pub fn resume_ref_from(prev: &Path) -> Option<Value> {
    let spec = parse_spec(prev);
    let layout = RunLayout::new(prev);
    let (_, r) =
        rustred_campaign_contract::classify::resume_state(&layout, &spec, &spec_digest(prev))
            .ok()?;
    Some(serde_json::to_value(r).unwrap())
}

/// Canonical record sequence (canonical lines) of a table file.
pub fn canon_lines(p: &Path) -> Vec<Vec<u8>> {
    let b = std::fs::read(p).unwrap();
    b.split(|&c| c == b'\n')
        .filter(|l| !l.is_empty())
        .map(|l| rustred_campaign_contract::canon::canonical_line(l).unwrap())
        .collect()
}

/// Our own processes still alive whose cmdline mentions rr_fake_child and the given marker path.
pub fn survivors(marker: &Path) -> Vec<i32> {
    let me = marker.to_str().unwrap().to_string();
    let mut v = vec![];
    for e in std::fs::read_dir("/proc").unwrap().flatten() {
        let Ok(pid) = e.file_name().to_string_lossy().parse::<i32>() else {
            continue;
        };
        let Ok(cmd) = std::fs::read(format!("/proc/{pid}/cmdline")) else {
            continue;
        };
        let cmd = String::from_utf8_lossy(&cmd).replace('\0', " ");
        if cmd.contains("rr_fake_child") && cmd.contains(&me) {
            if let Ok(s) = std::fs::read_to_string(format!("/proc/{pid}/stat")) {
                let after = &s[s.rfind(')').unwrap() + 2..];
                if after.starts_with('Z') {
                    continue;
                }
            }
            v.push(pid);
        }
    }
    v
}
