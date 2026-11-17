//! Shared helpers of the proc_* integration tests (real processes). Robust on a loaded host: generous timeouts, no
//! timing assertion tighter than seconds, one test at a time per binary ([`serial`]), a handful of processes each.
#![allow(dead_code)]

use rustred_campaign::launch::{ChildHandle, ChildProgress, FinalReport, LaunchSpec, RealLauncher};
use rustred_campaign::resources::{Sampler, SamplerConfig, scan_sessions_now};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

pub const LONG: Duration = Duration::from_secs(240);

/// One real-process test at a time in a test binary.
pub fn serial() -> MutexGuard<'static, ()> {
    static M: Mutex<()> = Mutex::new(());
    M.lock().unwrap_or_else(|p| p.into_inner())
}

/// A fresh temporary directory (removed by [`TempDir`]'s Drop).
pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new(name: &str) -> Self {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        // Unique on the host (procfix3 item 0): inside the private pid namespaces every test binary is pid 1.
        static TOKEN: std::sync::OnceLock<String> = std::sync::OnceLock::new();
        let token = TOKEN.get_or_init(|| {
            let ns = std::fs::read_link("/proc/self/ns/pid")
                .map(|l| l.to_string_lossy().into_owned())
                .unwrap_or_default();
            let stat = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
            let t = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            blake3::hash(format!("{ns}|{}|{stat}|{t}", std::process::id()).as_bytes()).to_hex()[..8]
                .to_string()
        });
        let p = std::env::temp_dir().join(format!("rr-proc-test-{token}-{name}-{n}"));
        std::fs::create_dir_all(std::env::temp_dir()).unwrap();
        std::fs::create_dir(&p).unwrap_or_else(|e| panic!("{}: {e} (not unique?)", p.display()));
        Self(p)
    }
    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub fn sampler() -> Arc<Sampler> {
    Arc::new(
        Sampler::spawn(SamplerConfig {
            interval: Duration::from_millis(200),
            collect_foreign_pinning: false,
            foreign_pin_max_cpus: usize::MAX,
        })
        .unwrap(),
    )
}

pub fn launcher() -> RealLauncher {
    RealLauncher::new(sampler())
}

/// `/bin/sh -c SCRIPT` with PATH copied from the test's environment.
pub fn sh(dir: &Path, label: &str, script: &str) -> LaunchSpec {
    let mut s = LaunchSpec::new(label, "/bin/sh", dir).arg("-c").arg(script);
    if let Some(p) = std::env::var_os("PATH") {
        s.env.push(("PATH".into(), p));
    }
    s
}

/// Poll until finished (panics after `LONG`).
pub fn drive<H: ChildHandle>(h: &mut H) -> FinalReport {
    let t0 = Instant::now();
    loop {
        match h.poll().unwrap() {
            ChildProgress::Finished(r) => return r,
            _ if t0.elapsed() > LONG => {
                panic!("child {} did not finish within {LONG:?}", h.label())
            }
            _ => std::thread::sleep(Duration::from_millis(50)),
        }
    }
}

pub fn wait_until(what: &str, timeout: Duration, mut cond: impl FnMut() -> bool) {
    let t0 = Instant::now();
    while !cond() {
        assert!(t0.elapsed() < timeout, "timed out waiting for: {what}");
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// No live process of session `sid` (retries: a just-killed process may still be exiting).
pub fn assert_session_gone(sid: u32) {
    let sids: BTreeSet<u32> = [sid].into();
    wait_until(
        &format!("session {sid} empty"),
        Duration::from_secs(60),
        || {
            scan_sessions_now(&sids)
                .get(&sid)
                .is_none_or(|u| u.members.is_empty())
        },
    );
}

/// A CPU this process may use and the site never forbids (168-175, 288-319).
pub fn usable_cpu() -> usize {
    let allowed = rustred_campaign::resources::allowed_cpus().unwrap();
    *allowed
        .iter()
        .rev()
        .find(|&&c| !(168..=175).contains(&c) && !(288..=319).contains(&c))
        .expect("an allowed CPU outside 168-175 and 288-319")
}

/// Re-run this test executable as a helper: `--exact <test> --nocapture` with `RR_PROC_HELPER=<mode>`.
pub fn helper_spec(dir: &Path, label: &str, test_name: &str, mode: &str) -> LaunchSpec {
    let exe = std::env::current_exe().unwrap();
    let mut s = LaunchSpec::new(label, exe, dir)
        .arg("--exact")
        .arg(test_name)
        .arg("--nocapture")
        .arg("--test-threads=1")
        .env("RR_PROC_HELPER", mode);
    if let Some(p) = std::env::var_os("PATH") {
        s.env.push(("PATH".into(), p));
    }
    // procfix7 round 7 (gate11 m11-tmp): inside a private namespace the helper supervisor's id lock lives under TMPDIR
    // (`launch::idlock::default_lock_dir`); without it a helper the test SIGKILLs left its ad-hoc lock file in the host
    // /tmp (one per run of proc_pdeathsig / proc_memory, proc_audit_regressions)
    if let Some(t) = std::env::var_os("TMPDIR") {
        s.env.push(("TMPDIR".into(), t));
    }
    s
}
