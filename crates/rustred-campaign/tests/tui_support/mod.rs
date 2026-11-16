//! Shared helpers of the dashboard tests (lane I-TUI).
#![allow(dead_code)]

pub mod demo;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rustred_campaign::clock::{ManualClock, UtcMillis};
use rustred_campaign::tui::{DashboardOptions, DirSource, MemorySource};
use serde_json::Value;

pub fn data() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data")
}

/// Hand fixture directory (a CDIR with `progress/`).
pub fn fixture_dir(name: &str) -> PathBuf {
    data().join("tui").join(name)
}

/// A DirSource on a hand fixture that SHOWS a checkout-independent label instead of the absolute path (audit M5:
/// golden screens must not contain machine- or checkout-specific text).
pub fn fixture_source(name: &str) -> DirSource {
    DirSource::new(fixture_dir(name)).with_display(format!("tests/data/tui/{name}"))
}

/// Text that would make a golden depend on the machine or the checkout.
pub fn machine_specific(text: &str) -> Option<String> {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let mut needles = vec![
        manifest.to_string(),
        "/shared/".into(),
        "/home/".into(),
        "/common/".into(),
    ];
    if let Ok(u) = std::env::var("USER")
        && !u.is_empty()
    {
        needles.push(format!("/{u}/"));
    }
    needles.into_iter().find(|n| text.contains(n.as_str()))
}

pub fn read_json(p: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(p).unwrap_or_else(|e| panic!("{}: {e}", p.display())))
        .unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// The as-built golden progress files of the ledger package, as a memory source.
pub fn ledger_golden_source() -> MemorySource {
    let d = data().join("ledger");
    let mut m = BTreeMap::new();
    m.insert(
        "campaign.json".to_string(),
        read_json(&d.join("progress-campaign.json")),
    );
    m.insert(
        "nodes/T162.json".to_string(),
        read_json(&d.join("progress-node-T162.json")),
    );
    m.insert(
        "events.json".to_string(),
        read_json(&d.join("progress-events.json")),
    );
    let mut s = MemorySource::new(m);
    s.details.insert(
        "T162-item-00004".into(),
        read_json(&d.join("progress-item-detail.json")),
    );
    s
}

/// Copies the ledger goldens into a CDIR layout under `dst`.
pub fn ledger_golden_cdir(dst: &Path) {
    let d = data().join("ledger");
    let p = dst.join("progress");
    std::fs::create_dir_all(p.join("nodes")).unwrap();
    std::fs::copy(d.join("progress-campaign.json"), p.join("campaign.json")).unwrap();
    std::fs::copy(d.join("progress-node-T162.json"), p.join("nodes/T162.json")).unwrap();
    std::fs::copy(d.join("progress-events.json"), p.join("events.json")).unwrap();
}

/// Recursively copies a directory.
pub fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for e in std::fs::read_dir(src).unwrap().flatten() {
        let to = dst.join(e.file_name());
        if e.path().is_dir() {
            copy_dir(&e.path(), &to);
        } else {
            std::fs::copy(e.path(), &to).unwrap();
        }
    }
}

/// The time a fixture is shown at: `fixture_now_utc`, else `written_utc` + 2 s.
pub fn fixture_now(campaign: &Value) -> UtcMillis {
    if let Some(t) = campaign.get("fixture_now_utc").and_then(Value::as_str) {
        return UtcMillis::parse_iso(t).unwrap();
    }
    let w = UtcMillis::parse_iso(campaign["written_utc"].as_str().unwrap()).unwrap();
    UtcMillis(w.0 + 2000)
}

pub fn opts_at(t: UtcMillis) -> (DashboardOptions, Arc<ManualClock>) {
    let clock = Arc::new(ManualClock::new(t));
    let o = DashboardOptions {
        clock: clock.clone(),
        utf8: Some(false),
        ..DashboardOptions::default()
    };
    (o, clock)
}

/// Options at the fixture's own time.
pub fn opts_for(campaign: &Value) -> DashboardOptions {
    opts_at(fixture_now(campaign)).0
}

/// A scratch directory unique per call on the host (lane int5): under `RR_TUI_SCRATCH` when set, else under TMPDIR
/// (light_build.sh sets a private one on the non-snapshotted build cache); never the pid alone, never removing a
/// directory it did not create.
pub fn scratch(name: &str) -> PathBuf {
    match std::env::var_os("RR_TUI_SCRATCH") {
        Some(b) => {
            let base = PathBuf::from(b);
            std::fs::create_dir_all(&base).unwrap();
            let u = unique_temp_dir(&format!("rr-tui-{name}"));
            let d = base.join(u.file_name().unwrap());
            let _ = std::fs::remove_dir(&u);
            std::fs::create_dir(&d).unwrap_or_else(|e| panic!("{}: {e}", d.display()));
            d
        }
        None => unique_temp_dir(&format!("rr-tui-{name}")),
    }
}

/// Compares `text` with the golden screen `tests/data/tui/screens/<name>.txt` (rewritten with
/// `RR_TUI_UPDATE_GOLDENS=1`).
pub fn check_golden(name: &str, text: &str) {
    if let Some(n) = machine_specific(text) {
        panic!("golden {name} would contain machine-specific text {n:?}:\n{text}");
    }
    let p = data().join("tui/screens").join(format!("{name}.txt"));
    if std::env::var_os("RR_TUI_UPDATE_GOLDENS").is_some() {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, text).unwrap();
        return;
    }
    let want = std::fs::read_to_string(&p).unwrap_or_else(|e| {
        panic!(
            "golden {} missing ({e}); run with RR_TUI_UPDATE_GOLDENS=1",
            p.display()
        )
    });
    if want != text {
        let diff: Vec<String> = want
            .lines()
            .zip(text.lines())
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .take(5)
            .map(|(i, (a, b))| format!("line {}:\n  want {a:?}\n  got  {b:?}", i + 1))
            .collect();
        panic!(
            "golden {name} differs ({} vs {} lines):\n{}\n--- got ---\n{text}",
            want.lines().count(),
            text.lines().count(),
            diff.join("\n")
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
