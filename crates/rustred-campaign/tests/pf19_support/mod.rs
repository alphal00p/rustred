//! Lane procfix7 round 19 (the coordinator's landing condition): run a caller of `RealLauncher::new` with THIS process's
//! ad-hoc id lock held by another open file description (every launcher of the process is then refused with
//! `IdLock(InUse)`), in a FRESH process of the test binary: the id lock is process-wide, so no other test may share the
//! process. The child writes its verdict to a file (a sibling test's adopted-children reaper may take its exit status).
#![allow(dead_code)]

use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

const OUT: &str = "RR_PF19_OUT";

/// This process's ad-hoc id lock, held (keep the file alive for the scenario).
pub fn hold_own_adhoc_lock() -> std::fs::File {
    let dir = rustred_campaign::launch::default_lock_dir();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    let f = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(dir.join(format!(
            "{}.lock",
            rustred_campaign::launch::adhoc_campaign_id()
        )))
        .unwrap();
    f.try_lock().unwrap();
    f
}

/// In the test process: run `test` alone in a fresh process of this binary and return the verdict it wrote. In that
/// child (None): run the scenario, then [`passed`].
pub fn in_fresh_process(test: &str) -> Option<String> {
    if std::env::var_os(OUT).is_some() {
        return None;
    }
    let out = std::env::temp_dir().join(format!(
        "rr-pf19-{test}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let st = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", test, "--nocapture", "--test-threads=1"])
        .env(OUT, &out)
        .stdin(std::process::Stdio::null())
        .status();
    Some(std::fs::read_to_string(&out).unwrap_or_else(|e| {
        format!("no verdict ({e}; child {st:?}): the scenario failed, see its output above")
    }))
}

/// The scenario passed (child): its verdict line.
pub fn passed(detail: &str) {
    std::fs::write(std::env::var_os(OUT).unwrap(), format!("PASS {detail}")).unwrap();
}
