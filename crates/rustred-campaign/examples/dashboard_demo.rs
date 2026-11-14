//! Demo host of the campaign dashboard (lane I-TUI). Renders any campaign directory (read-only) or a built-in fixture.
//!
//! ```text
//! dashboard_demo CDIR                              live read-only viewer of CDIR/progress (q or Ctrl-C closes it)
//! dashboard_demo --fixture midrun                  built-in fixture: golden | empty | midrun | finished | stale
//! dashboard_demo --fixture midrun --once [--width 200 --height 60]   one text rendering
//! dashboard_demo --fixture midrun --plain | --json
//! dashboard_demo --fixture midrun --supervisor [--grace 5]           Ctrl-C protocol as inside `run` (fake supervisor)
//! options: --color auto|always|never   --utf8
//! ```
//! Fixtures are shown at their own time (`fixture_now_utc`), advancing in real time, so ages and STALE look as recorded.
//! Exit: prints `exit reason: <reason>` on stderr after the terminal is restored (the pseudo-terminal test reads it).

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use rustred_campaign::clock::{Clock, UtcMillis};
use rustred_campaign::signals::{StopHandle, StopSignals};
use rustred_campaign::tui::{
    self, ColorChoice, Controls, DashboardOptions, DirSource, MemorySource, ProgressSource,
    SupervisorControls, WatchControls,
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

macro_rules! files {
    ($($rel:literal => $path:literal),* $(,)?) => {
        vec![$(($rel, include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/", $path)))),*]
    };
}

fn fixture(name: &str) -> Option<Vec<(&'static str, &'static str)>> {
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
        "midrun" => files![
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
        _ => return None,
    })
}

/// The time a fixture is shown at: `fixture_now_utc`, else `written_utc` + 2 s.
fn fixture_now(campaign: &Value) -> Option<UtcMillis> {
    if let Some(t) = campaign.get("fixture_now_utc").and_then(Value::as_str) {
        return UtcMillis::parse_iso(t);
    }
    let w = UtcMillis::parse_iso(campaign.get("written_utc")?.as_str()?)?;
    Some(UtcMillis(w.0 + 2000))
}

fn usage() -> ! {
    eprintln!(
        "usage: dashboard_demo (CDIR | --fixture golden|empty|midrun|finished|stale) [--once [--width W --height H]] [--plain] [--json] [--supervisor [--grace S]] [--color auto|always|never] [--utf8]"
    );
    std::process::exit(2)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let (mut cdir, mut fix) = (None::<String>, None::<String>);
    let (mut once, mut plain, mut json, mut sup, mut utf8) = (false, false, false, false, false);
    let (mut width, mut height, mut grace) = (120u16, 40u16, 5u64);
    let mut color = ColorChoice::Auto;
    while let Some(a) = args.next() {
        let mut val = || args.next().unwrap_or_else(|| usage());
        match a.as_str() {
            "--fixture" => fix = Some(val()),
            "--once" => once = true,
            "--plain" => plain = true,
            "--json" => json = true,
            "--supervisor" => sup = true,
            "--utf8" => utf8 = true,
            "--width" => width = val().parse().unwrap_or_else(|_| usage()),
            "--height" => height = val().parse().unwrap_or_else(|_| usage()),
            "--grace" => grace = val().parse().unwrap_or_else(|_| usage()),
            "--color" => {
                color = match val().as_str() {
                    "always" => ColorChoice::Always,
                    "never" => ColorChoice::Never,
                    "auto" => ColorChoice::Auto,
                    _ => usage(),
                }
            }
            s if s.starts_with('-') => usage(),
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
            let files = fixture(f).unwrap_or_else(|| usage());
            let mut map = BTreeMap::new();
            for (rel, text) in files {
                map.insert(
                    rel.to_string(),
                    serde_json::from_str::<Value>(text).expect("fixture is JSON"),
                );
            }
            if let Some(t) = map.get("campaign.json").and_then(fixture_now) {
                opts.clock = Arc::new(OffsetClock {
                    start: t,
                    origin: Instant::now(),
                });
            }
            Box::new(MemorySource::new(map))
        }
        _ => usage(),
    };
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&tui::view_model_json(source.as_ref(), &opts)).unwrap_or_default()
        );
        return;
    }
    if once {
        print!("{}", tui::render_once(source.as_ref(), &opts, width, height));
        return;
    }
    let reason = if sup {
        // a fake supervisor: the first stop request drains for `grace` seconds, the second ends at once
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
        tui::run_dashboard(source.as_ref(), c, &opts)
    } else {
        let controls = WatchControls::new();
        let _signals = StopSignals::install_into(controls.handle.clone()).ok();
        tui::run_dashboard(source.as_ref(), &controls, &opts)
    };
    eprintln!("exit reason: {reason:?}");
}
