//! Shared helpers of the dashboard tests (lane I-TUI).
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rustred_campaign::clock::{ManualClock, UtcMillis};
use rustred_campaign::tui::{DashboardOptions, MemorySource};
use serde_json::Value;

pub fn data() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data")
}

/// Hand fixture directory (a CDIR with `progress/`).
pub fn fixture_dir(name: &str) -> PathBuf {
    data().join("tui").join(name)
}

pub fn read_json(p: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(p).unwrap_or_else(|e| panic!("{}: {e}", p.display())))
        .unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// The as-built golden progress files of the ledger package, as a memory source.
pub fn ledger_golden_source() -> MemorySource {
    let d = data().join("ledger");
    let mut m = BTreeMap::new();
    m.insert("campaign.json".to_string(), read_json(&d.join("progress-campaign.json")));
    m.insert("nodes/T162.json".to_string(), read_json(&d.join("progress-node-T162.json")));
    m.insert("events.json".to_string(), read_json(&d.join("progress-events.json")));
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

/// A scratch directory under the non-snapshotted build cache (never under /shared or /home), unique per test.
pub fn scratch(name: &str) -> PathBuf {
    let base = std::env::var_os("RR_TUI_SCRATCH")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(format!(
                "/common/build-cache/{}/p_integral_dev/implementer-scratch/tui",
                std::env::var("USER").unwrap_or_else(|_| "user".into())
            ))
        });
    let base = if std::fs::create_dir_all(&base).is_ok() {
        base
    } else {
        std::env::temp_dir().join("rr-tui-tests")
    };
    let d = base.join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Compares `text` with the golden screen `tests/data/tui/screens/<name>.txt` (rewritten with
/// `RR_TUI_UPDATE_GOLDENS=1`).
pub fn check_golden(name: &str, text: &str) {
    let p = data().join("tui/screens").join(format!("{name}.txt"));
    if std::env::var_os("RR_TUI_UPDATE_GOLDENS").is_some() {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, text).unwrap();
        return;
    }
    let want = std::fs::read_to_string(&p).unwrap_or_else(|e| {
        panic!("golden {} missing ({e}); run with RR_TUI_UPDATE_GOLDENS=1", p.display())
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
