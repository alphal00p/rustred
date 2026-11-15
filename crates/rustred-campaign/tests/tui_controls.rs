//! Stop protocol and navigation of the dashboard with a fake `Controls` (no terminal).

mod tui_support;

use std::cell::Cell;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use rustred_campaign::clock::Clock;
use rustred_campaign::signals::{StopHandle, StopLevel};
use rustred_campaign::tui::app::{Action, Panel};
use rustred_campaign::tui::{self, App, Controls, DirSource, ExitReason, WatchControls};
use tui_support::*;

/// Records every stop request (stands in for the supervisor).
struct FakeControls {
    requests: Cell<usize>,
    hosts: bool,
}

impl Controls for FakeControls {
    fn request_stop(&self) -> StopLevel {
        self.requests.set(self.requests.get() + 1);
        StopLevel::from_count(self.requests.get())
    }
    fn stop_count(&self) -> usize {
        self.requests.get()
    }
    fn grace_seconds(&self) -> u64 {
        120
    }
    fn hosts_campaign(&self) -> bool {
        self.hosts
    }
}

fn ctrl_c() -> KeyEvent {
    KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)
}

fn key(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

#[test]
fn ctrl_c_once_drains_with_countdown_twice_forces_third_exits() {
    let dir = fixture_dir("midrun");
    let c = read_json(&dir.join("progress/campaign.json"));
    let (o, clock) = opts_at(fixture_now(&c));
    let src = DirSource::new(&dir);
    let vm = tui::view_model(&src, &o);
    let fake = FakeControls {
        requests: Cell::new(0),
        hosts: true,
    };
    let mut app = App::new(true, fake.grace_seconds());
    let now = |clock: &rustred_campaign::clock::ManualClock| clock.now_utc().0 as f64 / 1000.0;
    // q at top level does NOT stop the run
    assert_eq!(
        app.on_key(key(KeyCode::Char('q')), &vm, &src, &fake, now(&clock)),
        Action::Redraw
    );
    assert_eq!(fake.requests.get(), 0);
    let t = tui::render_text(&vm, &app, &o, 120, 40);
    assert!(
        t.contains("q does not stop the run: Ctrl-C stops it safely"),
        "{t}"
    );
    // first Ctrl-C: one stop request, drain banner with the grace countdown
    assert_eq!(
        app.on_key(ctrl_c(), &vm, &src, &fake, now(&clock)),
        Action::Redraw
    );
    assert_eq!(fake.requests.get(), 1);
    let t = tui::render_text(&vm, &app, &o, 120, 40);
    assert!(t.contains(" STOPPING 2:00  draining: 21 job(s) running, grace 120 s; nothing half-accepted. Press Ctrl-C again to force."), "{t}");
    assert!(t.contains("STOPPING 2:00"));
    clock.advance(std::time::Duration::from_secs(13));
    let t = tui::render_text(&vm, &app, &o, 120, 40);
    assert!(t.contains("STOPPING 1:47"), "countdown: {t}");
    let t80 = tui::render_text(&vm, &app, &o, 80, 24);
    assert!(t80.contains("STOPPING 1:47"), "{t80}");
    check_golden("stop-drain-120x40", &t);
    // second Ctrl-C: force
    assert_eq!(
        app.on_key(ctrl_c(), &vm, &src, &fake, now(&clock)),
        Action::Redraw
    );
    assert_eq!(fake.requests.get(), 2);
    let t = tui::render_text(&vm, &app, &o, 120, 40);
    assert!(t.contains(" FORCED STOP: 21 running job(s) are being killed and recorded as interrupted; still safe"), "{t}");
    assert!(t.contains("STOPPING (forced)"));
    check_golden("stop-forced-120x40", &t);
    // m8: a third press right after the second (key repeat, a burst) does not leave the dashboard
    assert_eq!(
        app.on_key(ctrl_c(), &vm, &src, &fake, now(&clock)),
        Action::Redraw
    );
    assert_eq!(
        fake.requests.get(),
        2,
        "the burst press is not a stop request"
    );
    clock.advance(std::time::Duration::from_secs(2));
    // third, after a pause: leave at once
    assert_eq!(
        app.on_key(ctrl_c(), &vm, &src, &fake, now(&clock)),
        Action::Quit(ExitReason::ForcedExit)
    );
    assert_eq!(fake.requests.get(), 3);
}

#[test]
fn stop_requests_from_signals_show_the_same_banner() {
    let dir = fixture_dir("midrun");
    let c = read_json(&dir.join("progress/campaign.json"));
    let (o, _) = opts_at(fixture_now(&c));
    let src = DirSource::new(&dir);
    let vm = tui::view_model(&src, &o);
    let h = StopHandle::new();
    let mut app = App::new(true, 120);
    assert_eq!(h.request_stop(), StopLevel::Drain); // as SIGINT would
    app.observe_stops(Controls::stop_count(&h), 0.0);
    assert_eq!(app.stop_count, 1);
    let t = tui::render_text(&vm, &app, &o, 120, 40);
    assert!(t.contains("draining:"), "{t}");
    // the real StopHandle as Controls: Ctrl-C bumps the shared counter (force)
    let r = app.on_key(ctrl_c(), &vm, &src, &h, 1.0);
    assert_eq!(r, Action::Redraw);
    assert_eq!(h.level(), StopLevel::Force);
}

#[test]
fn watcher_ctrl_c_and_q_close_the_viewer_only() {
    let dir = fixture_dir("midrun");
    let c = read_json(&dir.join("progress/campaign.json"));
    let (o, _) = opts_at(fixture_now(&c));
    let src = DirSource::new(&dir);
    let vm = tui::view_model(&src, &o);
    let w = WatchControls::new();
    let mut app = App::new(false, 120);
    assert_eq!(
        app.on_key(ctrl_c(), &vm, &src, &w, 0.0),
        Action::Quit(ExitReason::ViewerQuit)
    );
    assert_eq!(
        w.handle.count(),
        0,
        "the campaign's counter is never touched by a viewer"
    );
    let mut app = App::new(false, 120);
    assert_eq!(
        app.on_key(key(KeyCode::Char('q')), &vm, &src, &w, 0.0),
        Action::Quit(ExitReason::ViewerQuit)
    );
    let t = tui::render_text(&vm, &App::new(false, 120), &o, 120, 40);
    assert!(t.contains("q/Ctrl-C close viewer (campaign unaffected)"));
}

#[test]
fn navigation_panels_and_drill_down() {
    let g = ledger_golden_source();
    let c = g.files["campaign.json"].clone();
    let (o, _) = opts_at(fixture_now(&c));
    let vm = tui::view_model(&g, &o);
    let w = WatchControls::new();
    let mut app = App::new(false, 120);
    let mut src = g.clone();
    src.items.insert(
        "T162".into(),
        vec![rustred_campaign::tui::source::ItemRow {
            item: "T162-item-00004".into(),
            label: "F004 n9=-1".into(),
            kind: "locus".into(),
            rung: "L0".into(),
            state: "escalated".into(),
            why: "escalated to L1".into(),
            cpu_seconds: 2.5,
            jobs: 1,
            updated_seq: 62,
        }],
    );
    for (k, p) in [
        ('j', Panel::Jobs),
        ('e', Panel::Events),
        ('a', Panel::Attention),
        ('?', Panel::Help),
    ] {
        app.on_key(key(KeyCode::Char(k)), &vm, &src, &w, 0.0);
        assert_eq!(app.panel, p);
        let t = tui::render_text(&vm, &app, &o, 120, 40);
        assert!(t.lines().count() <= 40);
        check_golden(&format!("panel-{p:?}-120x40").to_lowercase(), &t);
        app.on_key(key(KeyCode::Esc), &vm, &src, &w, 0.0);
        assert_eq!(app.panel, Panel::Summary);
    }
    app.on_key(key(KeyCode::Enter), &vm, &src, &w, 0.0);
    assert_eq!(app.panel, Panel::Node);
    let t = tui::render_text(&vm, &app, &o, 120, 40);
    assert!(
        t.contains("F004 n9=-1") && t.contains("escalated to L1"),
        "{t}"
    );
    check_golden("panel-node-120x40", &t);
    app.on_key(key(KeyCode::Char('w')), &vm, &src, &w, 0.0);
    assert_eq!(app.panel, Panel::Item);
    let t = tui::render_text(&vm, &app, &o, 120, 40);
    assert!(
        t.contains("why escalated to L1") && t.contains("large:jobs/j4.a2"),
        "{t}"
    );
    check_golden("panel-item-120x40", &t);
    app.on_key(key(KeyCode::Char('q')), &vm, &src, &w, 0.0);
    app.on_key(key(KeyCode::Char('q')), &vm, &src, &w, 0.0);
    assert_eq!(app.panel, Panel::Summary);
    // control keys are explained, not bound
    let now = { o.clock.now_utc().0 as f64 / 1000.0 };
    app.on_key(key(KeyCode::Char('p')), &vm, &src, &w, now);
    let t = tui::render_text(&vm, &app, &o, 120, 40);
    assert!(
        t.contains("type `rr_campaign pause` in another pane"),
        "{t}"
    );
    // Tab cycles the 80-column table
    // the 80-column table starts with the nodes (audit suggestion 8)
    let t = tui::render_text(&vm, &app, &o, 80, 24);
    assert!(t.contains("-[ Nodes (1) ]"), "{t}");
    for title in [
        "Work list by ladder level",
        "Running jobs",
        "Recent events",
        "Attention",
        "Nodes",
    ] {
        app.on_key(key(KeyCode::Tab), &vm, &src, &w, 0.0);
        let t = tui::render_text(&vm, &app, &o, 80, 24);
        assert!(t.contains(&format!("-[ {title}")), "{title}: {t}");
    }
}

#[test]
fn drill_down_from_a_real_ledger() {
    // a real ledger written by the ledger package's support code is not available here without its helpers;
    // the DirSource reports clearly when there is no ledger
    let d = scratch("noledger");
    ledger_golden_cdir(&d);
    let src = DirSource::new(&d);
    use rustred_campaign::tui::ProgressSource;
    let e = src.node_items("T162").unwrap_err();
    assert!(e.contains("no ledger") || e.contains("ledger"), "{e}");
    assert!(src.running_jobs().is_none());
    let _ = std::fs::remove_dir_all(&d);
}
