//! Demo host of the campaign dashboard: ONE implementation shared by `examples/dashboard_demo.rs` and the pseudo-
//! terminal test (which runs it inside its own test binary, so no pre-built example is needed: audit M5).
//!
//! ```text
//! dashboard_demo CDIR                              live read-only viewer of CDIR/progress (q or Ctrl-C closes it)
//! dashboard_demo --fixture midrun                  built-in fixture: golden | empty | midrun | finished | stale | toy | probe
//! dashboard_demo --fixture midrun --once [--width 200 --height 60]   one text rendering
//! dashboard_demo --fixture midrun --plain | --json
//! dashboard_demo --fixture midrun --supervisor [--grace 5]           Ctrl-C protocol as inside `run` (fake supervisor)
//! options: --color auto|always|never   --utf8   --slow-read-ms N (every read blocks N ms: the screen stays responsive)
//! ```
//! Fixtures are shown at their own time (`fixture_now_utc`), advancing in real time, so ages and STALE look as
//! recorded. `probe` is `midrun` with terminal escape sequences injected into every kind of text (audit C1).
//! Exit: prints `exit reason: <reason>` on stderr after the terminal is restored (the pseudo-terminal test reads it).
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use rustred_campaign::clock::{Clock, UtcMillis};
use rustred_campaign::signals::{StopHandle, StopSignals};
use rustred_campaign::tui::source::{ItemRow, JobRow, RawSnapshot};
use rustred_campaign::tui::{
    self, ColorChoice, Controls, DashboardOptions, DirSource, MemorySource, ProgressSource,
    SupervisorControls,
};
use serde_json::Value;

struct OffsetClock {
    start: UtcMillis,
    origin: Instant,
}

impl Clock for OffsetClock {
    fn now_utc(&self) -> UtcMillis {
        UtcMillis(self.start.0 + self.origin.elapsed().as_millis() as i64)
    }
    fn monotonic(&self) -> Duration {
        self.origin.elapsed()
    }
}

/// A source whose reads block (a slow or hung file system).
pub struct SlowSource {
    pub inner: Box<dyn ProgressSource>,
    pub delay: Duration,
}

impl ProgressSource for SlowSource {
    fn read(&self) -> RawSnapshot {
        std::thread::sleep(self.delay);
        self.inner.read()
    }
    fn node_items(&self, node: &str) -> Result<Vec<ItemRow>, String> {
        std::thread::sleep(self.delay);
        self.inner.node_items(node)
    }
    fn item_detail(&self, item: &str) -> Result<Value, String> {
        std::thread::sleep(self.delay);
        self.inner.item_detail(item)
    }
    fn running_jobs(&self) -> Option<Vec<JobRow>> {
        self.inner.running_jobs()
    }
}

macro_rules! files {
    ($($rel:literal => $path:literal),* $(,)?) => {
        vec![$(($rel, include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/", $path)))),*]
    };
}

fn fixture_files(name: &str) -> Option<Vec<(&'static str, &'static str)>> {
    Some(match name {
        "golden" => files![
            "campaign.json" => "ledger/progress-campaign.json",
            "nodes/T162.json" => "ledger/progress-node-T162.json",
            "events.json" => "ledger/progress-events.json",
        ],
        "empty" => files![
            "campaign.json" => "tui/empty/progress/campaign.json",
            "events.json" => "tui/empty/progress/events.json",
        ],
        "midrun" | "probe" => files![
            "campaign.json" => "tui/midrun/progress/campaign.json",
            "events.json" => "tui/midrun/progress/events.json",
            "nodes/T074.json" => "tui/midrun/progress/nodes/T074.json",
            "nodes/T162.json" => "tui/midrun/progress/nodes/T162.json",
            "nodes/T178.json" => "tui/midrun/progress/nodes/T178.json",
            "nodes/T266.json" => "tui/midrun/progress/nodes/T266.json",
        ],
        "finished" => files![
            "campaign.json" => "tui/finished/progress/campaign.json",
            "events.json" => "tui/finished/progress/events.json",
            "nodes/T162.json" => "tui/finished/progress/nodes/T162.json",
            "nodes/T178.json" => "tui/finished/progress/nodes/T178.json",
            "nodes/T266.json" => "tui/finished/progress/nodes/T266.json",
        ],
        "stale" => files![
            "campaign.json" => "tui/stale/progress/campaign.json",
            "events.json" => "tui/stale/progress/events.json",
            "nodes/T162.json" => "tui/stale/progress/nodes/T162.json",
        ],
        "toy" => files![
            "campaign.json" => "tui/toy/progress/campaign.json",
            "events.json" => "tui/toy/progress/events.json",
            "nodes/kite.json" => "tui/toy/progress/nodes/kite.json",
            "nodes/sunset.json" => "tui/toy/progress/nodes/sunset.json",
        ],
        _ => return None,
    })
}

/// The injected sequences of the C1 probe: OSC 52 (write the clipboard), BEL, C1 CSI, alternate-screen exit, SGR,
/// bidi override, OSC 0 (window title).
pub const OSC52: &str = "\u{1b}]52;c;ZXZpbA==\u{7}";
pub const ALT_EXIT: &str = "\u{1b}[?1049l";
pub const C1_CSI: &str = "\u{9b}2J";
pub const TITLE: &str = "\u{1b}]0;PWNED\u{7}";
pub const BIDI: &str = "\u{202e}";

/// `midrun` with escape sequences in the campaign id, node ids (file names), item labels, run directories, job ids,
/// event texts, the ledger fallback's job rows, item ids and the drill-down error strings.
pub fn probe_source() -> MemorySource {
    let mut m = fixture_map("midrun");
    let c = m.get_mut("campaign.json").expect("campaign");
    c["campaign_id"] = Value::String(format!("camp{TITLE}{BIDI}x"));
    // job rows come from the ledger fallback path (no list in the progress files)
    c["jobs"].as_object_mut().expect("jobs").remove("list");
    let mut files = c["files"]["nodes"].clone();
    let evil = format!("T999{ALT_EXIT}{OSC52}");
    files[&evil] = Value::String(format!("nodes/{evil}.json"));
    c["files"]["nodes"] = files;
    let mut node = m["nodes/T178.json"].clone();
    node["node"] = Value::String(evil.clone());
    node["phase"] = Value::String(format!("running{C1_CSI}"));
    node["terminals"]["candidates"] = Value::from(1);
    m.insert(format!("nodes/{evil}.json"), node);
    if let Some(ev) = m
        .get_mut("events.json")
        .and_then(|e| e.get_mut("events"))
        .and_then(Value::as_array_mut)
        && let Some(last) = ev.last_mut()
    {
        last["text"] = Value::String(format!("event {OSC52} {ALT_EXIT} {C1_CSI} end"));
        last["kind"] = Value::String(format!("item_accepted{TITLE}"));
    }
    let mut src = MemorySource::new(m);
    src.jobs = Some(vec![JobRow {
        job: format!("J{OSC52}"),
        node: format!("N\u{1b}[31m{BIDI}"),
        item: format!("lab{C1_CSI}"),
        rung: "L0".into(),
        run_dir: Some(format!("d{ALT_EXIT}")),
        phase: Some(format!("run{TITLE}")),
        ..Default::default()
    }]);
    src.items.insert(
        "T074".into(),
        vec![ItemRow {
            item: format!("it{ALT_EXIT}"),
            label: format!("F1{OSC52}"),
            kind: "face".into(),
            rung: "L0".into(),
            state: "accepted".into(),
            why: format!("why{C1_CSI}"),
            cpu_seconds: 1.0,
            jobs: 1,
            updated_seq: 1,
        }],
    );
    src
}

pub fn fixture_map(name: &str) -> BTreeMap<String, Value> {
    let files = fixture_files(name).expect("known fixture");
    let mut map = BTreeMap::new();
    for (rel, text) in files {
        map.insert(
            rel.to_string(),
            serde_json::from_str::<Value>(text).expect("fixture is JSON"),
        );
    }
    map
}

/// The time a fixture is shown at: `fixture_now_utc`, else `written_utc` + 2 s.
fn fixture_now(campaign: &Value) -> Option<UtcMillis> {
    if let Some(t) = campaign.get("fixture_now_utc").and_then(Value::as_str) {
        return UtcMillis::parse_iso(t);
    }
    let w = UtcMillis::parse_iso(campaign.get("written_utc")?.as_str()?)?;
    Some(UtcMillis(w.0 + 2000))
}

fn usage() -> i32 {
    eprintln!(
        "usage: dashboard_demo (CDIR | --fixture golden|empty|midrun|finished|stale|toy|probe) [--once [--width W --height H]] [--plain] [--json] [--supervisor [--grace S]] [--color auto|always|never] [--utf8] [--slow-read-ms N]"
    );
    2
}

/// The demo's `main`, returning the exit code.
pub fn main_with(args: Vec<String>) -> i32 {
    let mut args = args.into_iter();
    let (mut cdir, mut fix) = (None::<String>, None::<String>);
    let (mut once, mut plain, mut json, mut sup, mut utf8) = (false, false, false, false, false);
    let (mut width, mut height, mut grace, mut slow) = (120u16, 40u16, 5u64, 0u64);
    let mut color = ColorChoice::Auto;
    while let Some(a) = args.next() {
        let mut val = || args.next();
        match a.as_str() {
            "--fixture" => fix = val(),
            "--once" => once = true,
            "--plain" => plain = true,
            "--json" => json = true,
            "--supervisor" => sup = true,
            "--utf8" => utf8 = true,
            "--width" | "--height" | "--grace" | "--slow-read-ms" => {
                let Some(n) = val().and_then(|v| v.parse::<u64>().ok()) else {
                    return usage();
                };
                match a.as_str() {
                    "--width" => width = n.min(u16::MAX as u64) as u16,
                    "--height" => height = n.min(u16::MAX as u64) as u16,
                    "--grace" => grace = n,
                    _ => slow = n,
                }
            }
            "--color" => {
                color = match val().as_deref() {
                    Some("always") => ColorChoice::Always,
                    Some("never") => ColorChoice::Never,
                    Some("auto") => ColorChoice::Auto,
                    _ => return usage(),
                }
            }
            s if s.starts_with('-') => return usage(),
            s => cdir = Some(s.to_string()),
        }
    }
    let mut opts = DashboardOptions {
        color,
        plain,
        utf8: utf8.then_some(true),
        ..DashboardOptions::default()
    };
    let source: Box<dyn ProgressSource> = match (&cdir, &fix) {
        (Some(d), None) => Box::new(DirSource::new(d)),
        (None, Some(f)) => {
            if fixture_files(f).is_none() {
                return usage();
            }
            let src = if f == "probe" {
                probe_source()
            } else {
                MemorySource::new(fixture_map(f))
            };
            if let Some(t) = src.files.get("campaign.json").and_then(fixture_now) {
                opts.clock = Arc::new(OffsetClock {
                    start: t,
                    origin: Instant::now(),
                });
            }
            Box::new(src)
        }
        _ => return usage(),
    };
    let source: Arc<dyn ProgressSource> = if slow > 0 {
        Arc::new(SlowSource {
            inner: source,
            delay: Duration::from_millis(slow),
        })
    } else {
        Arc::from(source)
    };
    if json {
        println!("{}", tui::view_model_json_text(source.as_ref(), &opts));
        return 0;
    }
    if once {
        print!(
            "{}",
            tui::render_once(source.as_ref(), &opts, width, height)
        );
        return 0;
    }
    let reason = if sup {
        // a fake supervisor: the first stop request drains for `grace` seconds, the second ends at once; signals are
        // the caller's job (as documented for run_dashboard)
        let handle = StopHandle::new();
        let _signals = StopSignals::install_into(handle.clone()).ok();
        let controls = SupervisorControls::new(handle.clone(), grace);
        let done = Arc::clone(&controls.done);
        std::thread::spawn(move || {
            let mut first: Option<Instant> = None;
            loop {
                let n = handle.count();
                if n >= 1 && first.is_none() {
                    first = Some(Instant::now());
                }
                let forced = n >= 2;
                if forced || first.is_some_and(|f| f.elapsed() >= Duration::from_secs(grace)) {
                    // a forced stop takes a moment too (kills recorded); let the banner be seen
                    std::thread::sleep(Duration::from_millis(if forced { 1500 } else { 0 }));
                    done.store(true, Ordering::SeqCst);
                    return;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        });
        let c: &dyn Controls = &controls;
        tui::run_dashboard(source, c, &opts)
    } else {
        // the stand-alone viewer entry: installs its own signal handlers (audit M7)
        tui::run_viewer_of(source, &opts)
    };
    eprintln!("exit reason: {reason:?}");
    0
}
