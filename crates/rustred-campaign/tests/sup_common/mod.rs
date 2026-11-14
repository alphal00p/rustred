#![allow(dead_code)]
//! Shared harness of the `sup_*` tests: real `rr_campaign` supervisors (children of the test) running real
//! `rr_fake_child` jobs, each test in its own temp dir, with checks read back from the ledger.

use rustred_campaign::ledger::{Event, JobEvent, Record, read_records};
use rustred_campaign::state::{CampaignState, FinishClass, reduce};
use rustred_campaign::supervisor::result::{FinalResult, final_result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

pub const SUP: &str = env!("CARGO_BIN_EXE_rr_campaign");
pub const FAKE: &str = env!("CARGO_BIN_EXE_rr_fake_child");
static N: AtomicU32 = AtomicU32::new(0);
static SERIAL: Mutex<()> = Mutex::new(());

/// One test at a time per test binary (keeps the number of children bounded on a loaded host).
pub fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|p| p.into_inner())
}

/// CPUs for the plain policy: 8 allowed CPUs outside 168-175 and 288-319 (the fake children barely use them).
pub fn cpus() -> String {
    let allowed = rustred_campaign::resources::allowed_cpus().unwrap();
    allowed
        .into_iter()
        .filter(|c| !(168..=175).contains(c) && !(288..=319).contains(c))
        .take(8)
        .map(|c| c.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

pub struct Tmp {
    pub path: PathBuf,
    pub campaign: String,
}

impl Tmp {
    pub fn new(tag: &str) -> Tmp {
        let n = N.fetch_add(1, Ordering::Relaxed);
        let campaign = format!("sup-{tag}-{}-{n}", std::process::id());
        let path = std::env::temp_dir().join(format!("rr-{campaign}"));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Tmp { path, campaign }
    }
    pub fn cdir(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        kill_marked(&self.campaign);
        if std::env::var_os("RR_SUP_KEEP").is_none() {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}

/// A synthetic spec (JSON value). `host_floor` is 100 MB here: the tests run inside the light-build scope (MemoryMax 2 GB),
/// whose headroom IS the effective available memory the admission sees (process-layer `MemInfo::effective_available`);
/// the 50 GB host rule itself is untouched (the whole scope stays far below it).
/// for this test; `synthetic` / `resources` entries can be overridden by the caller.
pub fn spec(
    t: &Tmp,
    synthetic: serde_json::Value,
    resources: serde_json::Value,
) -> serde_json::Value {
    let mut res = serde_json::json!({
        "policy": "plain", "cpus": cpus(), "max_jobs": 8, "mem_total": "4G", "disk_floor": "1G",
        "grace_seconds": 5, "sampler_ms": 300, "host_floor": "100M", "heartbeat_timeout_s": 60, "start_timeout_s": 120
    });
    for (k, v) in resources.as_object().unwrap() {
        res[k] = v.clone();
    }
    let mut syn = serde_json::json!({"items": 40, "nodes": 3, "rungs": 3, "seed": 11, "case_ms": [80, 250],
        "mem_cap_mb": 40, "heartbeat_s": 1, "stop_grace_s": 3});
    for (k, v) in synthetic.as_object().unwrap() {
        syn[k] = v.clone();
    }
    serde_json::json!({
        "schema": "rustred.campaign.spec.v1", "campaign": t.campaign,
        "programs": {"rr_fake_child": {"path": FAKE}},
        "resources": res, "progress": {"cadence_seconds": 1}, "synthetic": syn
    })
}

pub fn write_spec(t: &Tmp, name: &str, v: &serde_json::Value) -> PathBuf {
    let p = t.path.join(name);
    std::fs::write(&p, serde_json::to_vec_pretty(v).unwrap()).unwrap();
    p
}

/// Start `rr_campaign run` (child of the test; output into a log file in the temp dir).
pub fn start(spec: &Path, cdir: &Path, extra: &[&str], env: &[(&str, &str)]) -> Child {
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(cdir.with_extension("log"))
        .unwrap();
    let mut c = Command::new(SUP);
    c.arg("run")
        .arg("--spec")
        .arg(spec)
        .arg("--dir")
        .arg(cdir)
        .args(extra)
        .stdin(Stdio::null())
        .stdout(log.try_clone().unwrap())
        .stderr(log)
        .env_remove("RR_CAMPAIGN_FAILPOINT");
    for (k, v) in env {
        c.env(k, v);
    }
    c.spawn().unwrap()
}

/// Wait for exit with a generous bound; kills and panics on timeout.
pub fn wait(c: &mut Child, limit: Duration) -> i32 {
    let t0 = Instant::now();
    loop {
        if let Some(s) = c.try_wait().unwrap() {
            use std::os::unix::process::ExitStatusExt;
            return s.code().unwrap_or(128 + s.signal().unwrap_or(0));
        }
        if t0.elapsed() > limit {
            let _ = c.kill();
            let _ = c.wait();
            panic!("supervisor did not exit within {limit:?}");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

pub fn signal(c: &Child, sig: i32) {
    // SAFETY: our own child process (unreaped, so its pid is pinned).
    unsafe { libc::kill(c.id() as i32, sig) };
}

/// Run to the end (resuming as often as needed); returns the exit codes of the sessions.
pub fn run_to_end(spec: &Path, cdir: &Path, extra: &[&str]) -> Vec<i32> {
    let mut codes = Vec::new();
    for _ in 0..6 {
        let mut c = start(spec, cdir, extra, &[]);
        let code = wait(&mut c, Duration::from_secs(900));
        codes.push(code);
        if code == 0 {
            return codes;
        }
    }
    panic!(
        "campaign did not finish: exit codes {codes:?}; log {}",
        log_tail(cdir)
    );
}

pub fn log_tail(cdir: &Path) -> String {
    let s = std::fs::read_to_string(cdir.with_extension("log")).unwrap_or_default();
    let lines: Vec<&str> = s.lines().collect();
    lines[lines.len().saturating_sub(40)..].join("\n")
}

pub fn records(cdir: &Path) -> Vec<Record> {
    read_records(cdir).unwrap()
}

pub fn state(cdir: &Path) -> CampaignState {
    reduce(records(cdir).iter()).unwrap()
}

pub fn result(cdir: &Path) -> FinalResult {
    final_result(&state(cdir))
}

/// Pids (of our uid) whose environment carries a marker of campaign id `campaign` (any campaign key of it).
pub fn marked(campaign: &str) -> Vec<u32> {
    let needle = format!("RR_CAMPAIGN_JOB={campaign}.");
    let mut out = Vec::new();
    for e in std::fs::read_dir("/proc").unwrap().flatten() {
        let Some(pid) = e.file_name().to_str().and_then(|s| s.parse::<u32>().ok()) else {
            continue;
        };
        let Ok(env) = std::fs::read(format!("/proc/{pid}/environ")) else {
            continue;
        };
        let st = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap_or_default();
        if st.rfind(')').is_some_and(|i| st[i + 2..].starts_with('Z')) {
            continue;
        }
        if env
            .split(|&b| b == 0)
            .any(|kv| kv.starts_with(needle.as_bytes()))
        {
            out.push(pid);
        }
    }
    out
}

/// Assert that no process carrying this test's campaign marker survives (bounded wait for exits in flight).
pub fn assert_no_marked(campaign: &str) {
    assert_no_marked_except(campaign, &[]);
}

/// As [`assert_no_marked`], ignoring planted decoys.
pub fn assert_no_marked_except(campaign: &str, decoys: &[u32]) {
    let t0 = Instant::now();
    loop {
        let m: Vec<u32> = marked(campaign)
            .into_iter()
            .filter(|p| !decoys.contains(p))
            .collect();
        if m.is_empty() {
            return;
        }
        if t0.elapsed() > Duration::from_secs(10) {
            kill_marked(campaign);
            panic!("marked processes survive: {m:?}");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Clean-up only (Drop): SIGKILL processes that carry a marker of THIS test's campaign (any marker id derived from
/// its unique campaign name), through the process layer's verified kill (no signal path of the tests' own).
pub fn kill_marked(campaign: &str) {
    let needle = format!("RR_CAMPAIGN_JOB={campaign}.");
    let mut ids = std::collections::BTreeSet::new();
    for p in marked(campaign) {
        if let Ok(env) = std::fs::read(format!("/proc/{p}/environ")) {
            for kv in env.split(|&b| b == 0) {
                if let Some(v) = kv.strip_prefix(b"RR_CAMPAIGN_JOB=")
                    && kv.starts_with(needle.as_bytes())
                    && let Some((id, _)) =
                        std::str::from_utf8(v).ok().and_then(|v| v.split_once(':'))
                {
                    ids.insert(id.to_string());
                }
            }
        }
    }
    let boot = rustred_campaign::resources::boot_id().unwrap_or_default();
    for id in ids {
        if let Ok(scan) = rustred_campaign::resources::scan_campaign(&id, 0, &Default::default()) {
            for m in &scan.processes {
                let _ = rustred_campaign::launch::kill_orphan(&id, &boot, m, libc::SIGKILL);
            }
        }
    }
}

/// Ledger-level invariants of a finished campaign: chain verifies (read_records + reduce), no item accepted twice,
/// re-runs bounded by interruptions, admission within budgets at every point of the record order.
pub struct LedgerFacts {
    pub interrupted: usize,
    pub requeues: usize,
    pub max_cores: usize,
    pub max_reserved: u64,
}

pub fn ledger_facts(cdir: &Path) -> LedgerFacts {
    let recs = records(cdir);
    let mut accepts: BTreeMap<String, usize> = BTreeMap::new();
    let mut running: BTreeMap<String, (usize, u64)> = BTreeMap::new();
    let (mut interrupted, mut requeues, mut max_cores, mut max_reserved) = (0, 0, 0usize, 0u64);
    for r in &recs {
        match &r.event {
            Event::Item {
                item,
                ev: rustred_campaign::ledger::ItemEvent::Accept { .. },
            } => *accepts.entry(item.clone()).or_default() += 1,
            Event::Job { job, attempt, ev } => {
                let key = format!("{job}:{attempt}");
                match ev {
                    JobEvent::Launch {
                        cpus,
                        reserved_bytes,
                        ..
                    } => {
                        running.insert(key, (cpus.len(), *reserved_bytes));
                    }
                    JobEvent::Finished { class, .. } => {
                        if matches!(class, FinishClass::Interrupted | FinishClass::Lost) {
                            interrupted += 1;
                        }
                        running.remove(&key);
                    }
                    JobEvent::Interrupt { .. } | JobEvent::Lose { .. } => {
                        interrupted += 1;
                        running.remove(&key);
                    }
                    JobEvent::Requeue { .. } => requeues += 1,
                    _ => {}
                }
                max_cores = max_cores.max(running.values().map(|x| x.0).sum());
                max_reserved = max_reserved.max(running.values().map(|x| x.1).sum());
            }
            _ => {}
        }
    }
    for (item, n) in &accepts {
        assert_eq!(*n, 1, "item {item} accepted {n} times");
    }
    assert!(
        running.is_empty(),
        "attempts never settled: {:?}",
        running.keys().collect::<Vec<_>>()
    );
    LedgerFacts {
        interrupted,
        requeues,
        max_cores,
        max_reserved,
    }
}

/// `rr_campaign verify CDIR` exit code.
pub fn verify(cdir: &Path) -> i32 {
    Command::new(SUP)
        .arg("verify")
        .arg(cdir)
        .stdout(Stdio::null())
        .status()
        .unwrap()
        .code()
        .unwrap_or(-1)
}

/// splitmix64 for seeded test choices.
pub fn mix(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// Count of ledger records matching a predicate.
pub fn count(cdir: &Path, f: impl Fn(&Event) -> bool) -> usize {
    records(cdir).iter().filter(|r| f(&r.event)).count()
}

/// Notes whose text starts with `prefix`.
pub fn notes(cdir: &Path, prefix: &str) -> Vec<String> {
    records(cdir)
        .into_iter()
        .filter_map(|r| match r.event {
            Event::Note { text } if text.starts_with(prefix) => Some(text),
            _ => None,
        })
        .collect()
}

/// Wait until `f` holds (generous bound), else panic.
pub fn wait_until(what: &str, limit: Duration, mut f: impl FnMut() -> bool) {
    let t0 = Instant::now();
    while !f() {
        if t0.elapsed() > limit {
            panic!("timed out waiting for {what}");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// `rr_campaign <args>` exit code (stdout captured and returned).
pub fn cli(args: &[&str]) -> (i32, String) {
    let o = Command::new(SUP)
        .args(args)
        .stderr(Stdio::inherit())
        .output()
        .unwrap();
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).into_owned(),
    )
}

/// A long-lived decoy process of the test (own session, cwd `cwd`, optional environment pair).
pub fn decoy(cwd: &Path, env: Option<(&str, &str)>) -> Child {
    use std::os::unix::process::CommandExt;
    let mut c = Command::new(sleep_bin());
    c.arg("600")
        .current_dir(cwd)
        .env_clear()
        .stdin(Stdio::null());
    if let Some((k, v)) = env {
        c.env(k, v);
    }
    // SAFETY: setsid is async-signal-safe.
    unsafe {
        c.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    c.spawn().unwrap()
}

pub fn start_ticks(pid: u32) -> u64 {
    let s = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
    s[s.rfind(')').unwrap() + 2..]
        .split_whitespace()
        .nth(19)
        .unwrap()
        .parse()
        .unwrap()
}

/// Absolute path of `sleep` from PATH (no /bin/sleep on every host).
pub fn sleep_bin() -> PathBuf {
    std::env::var_os("PATH")
        .and_then(|p| {
            std::env::split_paths(&p)
                .map(|d| d.join("sleep"))
                .find(|f| f.is_file())
        })
        .unwrap_or_else(|| PathBuf::from("/bin/sleep"))
}
