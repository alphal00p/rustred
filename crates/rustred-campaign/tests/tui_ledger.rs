//! The dashboard on a REAL campaign directory: a ledger written by the ledger package's scenario, progress files
//! written by `ProgressWriter`, then read by `DirSource` (summary from the files, drill-down from a read-only ledger
//! reduction). Uses the ledger package's test helpers read-only.

#[path = "ledger_support/mod.rs"]
mod ledger_support;
mod tui_support;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use rustred_campaign::clock::Clock;
use rustred_campaign::ledger::LedgerConfig;
use rustred_campaign::progress::{self, ProgressWriter, Resources};
use rustred_campaign::state::EvidenceLevel;
use rustred_campaign::tui::app::Panel;
use rustred_campaign::tui::{self, App, DirSource, ProgressSource, WatchControls};

fn resources() -> Resources {
    Resources {
        cpus_in_use: Some(21),
        cpus_allowed: Some(48),
        live_jobs: Some(3),
        reserved_bytes: Some(3 << 30),
        measured_rss_bytes: Some(1_234_567_890),
        mem_cap_bytes: Some(30 << 30),
        host_mem_available_bytes: Some(268 << 30),
        host_floor_bytes: Some(50 << 30),
        sampled_utc: Some("2026-11-13T23:10:00.000Z".into()),
        ..Default::default()
    }
}

#[test]
fn real_ledger_and_progress_files_summary_and_drill_down() {
    let d = tui_support::scratch("realledger");
    let clk = ledger_support::clock();
    let mut l =
        ledger_support::new_ledger(&d, ledger_support::as_dyn(&clk), LedgerConfig::default());
    l.append_all(ledger_support::scenario()).unwrap();
    for k in 10..13 {
        l.append_all(
            ledger_support::item_life("T162", k, 0, EvidenceLevel::IdentityTested, 1500)[..1]
                .to_vec(),
        )
        .unwrap();
    }
    let c: &dyn Clock = clk.as_ref();
    ProgressWriter::new()
        .write(&d, l.state(), l.head(), &resources(), c)
        .unwrap();
    let campaign = tui_support::read_json(&d.join("progress/campaign.json"));
    let o = tui_support::opts_for(&campaign);
    let src = DirSource::new(&d);
    // summary: website = terminal on the real file
    let screen = tui::render_once(&src, &o, 120, 40);
    let got = tui::screen_numbers(&screen);
    for (k, v) in tui::campaign_json_numbers(&campaign) {
        assert_eq!(got.get(&k), Some(&v), "{k}\n{screen}");
    }
    // drill-down from the ledger
    let items = src.node_items("T162").unwrap();
    assert!(items.len() >= 10, "{items:?}");
    let it4 = items
        .iter()
        .find(|i| i.item == ledger_support::item_id("T162", 4))
        .unwrap();
    assert_eq!(it4.why, "escalated to L1");
    let detail = src
        .item_detail(&ledger_support::item_id("T162", 4))
        .unwrap();
    assert_eq!(
        detail,
        progress::item_detail(l.state(), &ledger_support::item_id("T162", 4)).unwrap()
    );
    assert_eq!(src.running_jobs(), Some(vec![]));
    // keyboard path: Enter on the node, select item 4, w
    let vm = tui::view_model(&src, &o);
    assert_eq!(vm.jobs.list_origin, tui::model::JobsOrigin::LedgerPartial);
    let w = WatchControls::new();
    let mut app = App::new(false, 120);
    let k = |c: KeyCode| KeyEvent::new(c, KeyModifiers::NONE);
    app.on_key(k(KeyCode::Enter), &vm, &src, &w, 0.0);
    assert_eq!(app.panel, Panel::Node);
    let pos = app
        .visible_items()
        .iter()
        .position(|i| i.item.as_str() == it4.item)
        .unwrap();
    for _ in 0..pos {
        app.on_key(k(KeyCode::Down), &vm, &src, &w, 0.0);
    }
    app.on_key(k(KeyCode::Char('w')), &vm, &src, &w, 0.0);
    assert_eq!(app.panel, Panel::Item);
    let t = tui::render_text(&vm, &app, &o, 120, 40);
    assert!(t.contains("why escalated to L1"), "{t}");
    drop(l);
    let _ = std::fs::remove_dir_all(&d);
}
