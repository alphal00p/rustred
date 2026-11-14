//! Golden screens of the dashboard (TestBackend) at 80x24, 120x40, 200x60 on the ledger package's golden progress
//! files and the hand fixtures; colours of cells; resize sequence; plain-mode lines; "website = terminal".

mod tui_support;

use std::time::{Duration, Instant};

use ratatui::style::Color;
use rustred_campaign::tui::{
    self, App, DirSource, MemorySource, ProgressSource, ViewModel, model::Banner, plain,
};
use serde_json::Value;
use tui_support::*;

const SIZES: [(u16, u16); 3] = [(80, 24), (120, 40), (200, 60)];

fn vm_of(src: &dyn ProgressSource, campaign: &Value) -> (ViewModel, tui::DashboardOptions) {
    let o = opts_for(campaign);
    (tui::view_model(src, &o), o)
}

#[test]
fn golden_screens_of_every_fixture_at_three_widths() {
    let golden = ledger_golden_source();
    let gc = golden.files["campaign.json"].clone();
    for (w, h) in SIZES {
        let o = opts_for(&gc);
        check_golden(&format!("ledger-golden-{w}x{h}"), &tui::render_once(&golden, &o, w, h));
    }
    for name in ["empty", "midrun", "finished", "stale"] {
        let dir = fixture_dir(name);
        let c = read_json(&dir.join("progress/campaign.json"));
        let src = DirSource::new(&dir);
        for (w, h) in SIZES {
            let o = opts_for(&c);
            check_golden(&format!("{name}-{w}x{h}"), &tui::render_once(&src, &o, w, h));
        }
    }
}

#[test]
fn every_line_fits_and_nothing_wraps() {
    for name in ["empty", "midrun", "finished", "stale"] {
        let dir = fixture_dir(name);
        let c = read_json(&dir.join("progress/campaign.json"));
        let src = DirSource::new(&dir);
        for (w, h) in SIZES {
            let text = tui::render_once(&src, &opts_for(&c), w, h);
            assert!(text.lines().count() <= h as usize, "{name} {w}x{h}");
            for l in text.lines() {
                assert!(l.chars().count() <= w as usize, "{name} {w}x{h}: {l}");
            }
        }
    }
}

fn find(buf: &ratatui::buffer::Buffer, needle: &str) -> Option<(u16, u16)> {
    let a = buf.area;
    for y in a.y..a.bottom() {
        let line: String = (a.x..a.right()).map(|x| buf[(x, y)].symbol()).collect();
        if let Some(p) = line.find(needle) {
            return Some((line[..p].chars().count() as u16, y));
        }
    }
    None
}

#[test]
fn boxed_rows_are_blue_full_green_stale_yellow_and_colour_off_has_no_colour() {
    let dir = fixture_dir("midrun");
    let c = read_json(&dir.join("progress/campaign.json"));
    let (vm, o) = vm_of(&DirSource::new(&dir), &c);
    let now = o.clock.now_utc().0 as f64 / 1000.0;
    let app = App::new(false, 120);
    let buf = tui::render_to_buffer(&vm, &app, true, false, now, 120, 40);
    let (x, y) = find(&buf, "[BOXED T162.rstar]").expect("boxed tag shown");
    assert_eq!(buf[(x, y)].fg, Color::Blue, "BOXED tag blue (2.4.15)");
    // the whole BOXED row is blue (node id cell)
    let (x2, y2) = find(&buf, "T162     [BOXED").unwrap();
    assert_eq!(buf[(x2, y2)].fg, Color::Blue);
    let (x, y) = find(&buf, "FULL  ").expect("FULL shown");
    assert_eq!(buf[(x, y)].fg, Color::Green);
    let (x, y) = find(&buf, "RUNNING").unwrap();
    assert_eq!(buf[(x, y)].fg, Color::Green);
    // host MemAvailable 60 G vs floor 50 G: within 25 % of the floor -> yellow
    let (x, y) = find(&buf, "60.0G (floor").unwrap();
    assert_eq!(buf[(x, y)].fg, Color::Yellow);
    // no background colours anywhere, only named colours
    let a = buf.area;
    for yy in a.y..a.bottom() {
        for xx in a.x..a.right() {
            let cell = &buf[(xx, yy)];
            assert_eq!(cell.bg, Color::Reset, "no background colour at {xx},{yy}");
            assert!(!matches!(cell.fg, Color::Rgb(..) | Color::Indexed(_)), "named colours only");
        }
    }
    let plain = tui::render_to_buffer(&vm, &app, false, false, now, 120, 40);
    for yy in a.y..a.bottom() {
        for xx in a.x..a.right() {
            assert_eq!(plain[(xx, yy)].fg, Color::Reset, "colour off: no foreground colour");
        }
    }
    // STALE banner yellow
    let sdir = fixture_dir("stale");
    let sc = read_json(&sdir.join("progress/campaign.json"));
    let (svm, so) = vm_of(&DirSource::new(&sdir), &sc);
    assert_eq!(svm.banner, Banner::Stale);
    let sbuf = tui::render_to_buffer(&svm, &app, true, false, so.clock.now_utc().0 as f64 / 1000.0, 120, 40);
    let (x, y) = find(&sbuf, " STALE: no heartbeat for 10m06s").expect("stale banner");
    assert_eq!(sbuf[(x + 1, y)].fg, Color::Yellow);
}

#[test]
fn resize_sequence_never_panics_and_degrades() {
    let dir = fixture_dir("midrun");
    let c = read_json(&dir.join("progress/campaign.json"));
    let (vm, o) = vm_of(&DirSource::new(&dir), &c);
    let now = o.clock.now_utc().0 as f64 / 1000.0;
    let mut app = App::new(true, 120);
    app.stop_count = 1;
    app.stop_first_s = Some(now - 13.0);
    for (w, h) in [
        (1, 1), (2, 1), (10, 3), (79, 23), (80, 23), (79, 24), (80, 24), (100, 30), (119, 40), (120, 40),
        (150, 50), (199, 60), (200, 60), (300, 100), (80, 5), (200, 10), (40, 80),
    ] {
        let t = tui::buffer_text(&tui::render_to_buffer(&vm, &app, true, true, now, w, h));
        assert!(t.lines().count() <= h as usize);
        if w < 80 || h < 24 {
            // plain status line fallback
            assert!(t.contains("STOPPI") || w < 8, "{w}x{h}: {t}");
            if h >= 3 && w >= 40 {
                assert!(t.contains("enlarge to 80x24"), "{w}x{h}: {t}");
            }
        }
    }
    let t = tui::buffer_text(&tui::render_to_buffer(&vm, &app, false, false, now, 79, 23));
    assert!(t.starts_with("2026-11-20T08:15:06Z STOPPING seq 18,402 cpu 21/32"), "{t}");
}

#[test]
fn plain_mode_golden_lines() {
    let golden = ledger_golden_source();
    let gc = golden.files["campaign.json"].clone();
    let (vm, o) = vm_of(&golden, &gc);
    let app = App::new(false, 120);
    let mut pr = plain::PlainPrinter::new();
    let now = o.clock.now_utc().0 as f64 / 1000.0;
    let mut out = pr.lines(&vm, &app, now, 60.0);
    // nothing new 10 s later; a status line again after 60 s; a state change prints at once
    assert!(pr.lines(&vm, &app, now + 10.0, 60.0).is_empty());
    out.extend(pr.lines(&vm, &app, now + 61.0, 60.0));
    let mut stopping = app.clone();
    stopping.stop_count = 1;
    out.extend(pr.lines(&vm, &stopping, now + 62.0, 60.0));
    let midrun = DirSource::new(fixture_dir("midrun"));
    let mc = read_json(&fixture_dir("midrun/progress/campaign.json"));
    let (mvm, _) = vm_of(&midrun, &mc);
    out.push(plain::status_line(&mvm, &app));
    let stale = DirSource::new(fixture_dir("stale"));
    let sc = read_json(&fixture_dir("stale/progress/campaign.json"));
    let (svm, _) = vm_of(&stale, &sc);
    out.push(plain::status_line(&svm, &app));
    let text = out.join("\n") + "\n";
    for l in text.lines() {
        assert!(!l.contains('\u{1b}'), "no escape codes: {l}");
    }
    check_golden("plain-lines", &text);
}

#[test]
fn summary_numbers_equal_campaign_json_website_equals_terminal() {
    let mut cases: Vec<(String, Box<dyn ProgressSource>, Value)> = Vec::new();
    let g = ledger_golden_source();
    let gc = g.files["campaign.json"].clone();
    cases.push(("ledger-golden".into(), Box::new(g), gc));
    for name in ["empty", "midrun", "finished", "stale"] {
        let d = fixture_dir(name);
        let c = read_json(&d.join("progress/campaign.json"));
        cases.push((name.into(), Box::new(DirSource::new(&d)), c));
    }
    for (name, src, c) in cases {
        for (w, h) in [(120u16, 40u16), (200, 60)] {
            let screen = tui::render_once(src.as_ref(), &opts_for(&c), w, h);
            let got = tui::screen_numbers(&screen);
            let want = tui::campaign_json_numbers(&c);
            for (k, v) in &want {
                assert_eq!(got.get(k), Some(v), "{name} {w}x{h}: {k}\n{screen}");
            }
        }
    }
}

#[test]
fn absent_garbage_half_written_and_unknown_major_files() {
    let d = scratch("garbage");
    // absent directory
    let t = tui::render_once(&DirSource::new(d.join("nope")), &opts_at(Default::default()).0, 120, 40);
    assert!(t.contains("NO DATA") && t.contains("campaign.json absent"), "{t}");
    // half-written / garbage campaign.json
    std::fs::create_dir_all(d.join("progress/nodes")).unwrap();
    std::fs::write(d.join("progress/campaign.json"), b"{\"schema\":\"rustred.campaign.progress.v1\",\"campa").unwrap();
    let src = DirSource::new(&d);
    let t = tui::render_once(&src, &opts_at(Default::default()).0, 120, 40);
    assert!(t.contains("UNREADABLE") && t.contains("not JSON"), "{t}");
    // unknown major
    std::fs::write(d.join("progress/campaign.json"), b"{\"schema\":\"rustred.campaign.progress.v2\"}").unwrap();
    let t = tui::render_once(&src, &opts_at(Default::default()).0, 120, 40);
    assert!(t.contains("schema v2 not understood"), "{t}");
    // a good file, then a torn one: the last good copy is kept and the problem shown
    let good = std::fs::read(fixture_dir("midrun/progress/campaign.json")).unwrap();
    std::fs::write(d.join("progress/campaign.json"), &good).unwrap();
    let c: Value = serde_json::from_slice(&good).unwrap();
    let o = opts_for(&c);
    let t1 = tui::render_once(&src, &o, 120, 40);
    assert!(t1.contains("RUNNING"));
    std::fs::write(d.join("progress/campaign.json"), &good[..good.len() / 2]).unwrap();
    let t2 = tui::render_once(&src, &o, 120, 40);
    assert!(t2.contains("RUNNING") && t2.contains("campaign.json unreadable"), "{t2}");
    // garbage node and events files, binary junk, wrong types
    std::fs::write(d.join("progress/nodes/T162.json"), [0xff, 0x00, 0x1b, b'[']).unwrap();
    std::fs::write(d.join("progress/events.json"), b"{\"schema\":\"rustred.campaign.progress.v1\",\"events\":[{\"seq\":-1,\"utc\":5,\"kind\":{},\"text\":\"\\u001b[31mred\"}]}").unwrap();
    let t3 = tui::render_once(&src, &o, 120, 40);
    assert!(!t3.contains('\u{1b}'));
    assert!(t3.contains("unreadable"), "{t3}");
    let vm = tui::view_model(&src, &o);
    assert!(vm.problems.iter().any(|p| p.starts_with("node T162: not JSON")), "{:?}", vm.problems);
    assert!(vm.events.iter().all(|e| !e.text.contains('\u{1b}')));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn read_only_campaign_directory_works() {
    let d = scratch("readonly");
    copy_dir(&fixture_dir("midrun"), &d);
    let set_ro = |p: &std::path::Path, ro: bool| {
        use std::os::unix::fs::PermissionsExt;
        for e in walk(p) {
            let mode = if e.is_dir() { if ro { 0o555 } else { 0o755 } } else if ro { 0o444 } else { 0o644 };
            std::fs::set_permissions(&e, std::fs::Permissions::from_mode(mode)).unwrap();
        }
    };
    set_ro(&d, true);
    let before = listing(&d);
    let c = read_json(&d.join("progress/campaign.json"));
    let src = DirSource::new(&d);
    let o = opts_for(&c);
    let t = tui::render_once(&src, &o, 120, 40);
    let _ = tui::view_model_json(&src, &o);
    let _ = src.node_items("T162");
    assert!(t.contains("RUNNING") && t.contains("[BOXED T162.rstar]"), "{t}");
    assert_eq!(before, listing(&d), "nothing written");
    set_ro(&d, false);
    let _ = std::fs::remove_dir_all(&d);
}

fn walk(p: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut v = vec![p.to_path_buf()];
    if p.is_dir() {
        for e in std::fs::read_dir(p).unwrap().flatten() {
            v.extend(walk(&e.path()));
        }
    }
    v
}

fn listing(p: &std::path::Path) -> Vec<(String, u64)> {
    let mut v: Vec<(String, u64)> = walk(p)
        .into_iter()
        .map(|e| (e.display().to_string(), std::fs::metadata(&e).map(|m| m.len()).unwrap_or(0)))
        .collect();
    v.sort();
    v
}

#[test]
fn thirty_thousand_items_render_windowed_and_fast() {
    use rustred_campaign::tui::source::ItemRow;
    let dir = fixture_dir("midrun");
    let c = read_json(&dir.join("progress/campaign.json"));
    let mut m = std::collections::BTreeMap::new();
    for rel in ["campaign.json", "events.json", "nodes/T074.json", "nodes/T162.json", "nodes/T178.json", "nodes/T266.json"] {
        m.insert(rel.to_string(), read_json(&dir.join("progress").join(rel)));
    }
    let mut src = MemorySource::new(m);
    let items: Vec<ItemRow> = (0..30_000)
        .map(|i| ItemRow {
            item: format!("T178-item-{i:05}"),
            label: format!("F{:03} n9=-1", i % 997),
            kind: "face".into(),
            rung: format!("L{}", i % 5),
            state: ["open", "accepted", "escalated", "in_progress"][i % 4].into(),
            why: "accepted at identity_proved".into(),
            cpu_seconds: i as f64 / 10.0,
            jobs: 1,
            updated_seq: i as u64,
        })
        .collect();
    src.items.insert("T178".into(), items);
    let o = opts_for(&c);
    let vm = tui::view_model(&src, &o);
    let controls = rustred_campaign::tui::WatchControls::new();
    let mut app = App::new(false, 120);
    let now = o.clock.now_utc().0 as f64 / 1000.0;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let key = |c: KeyCode| KeyEvent::new(c, KeyModifiers::NONE);
    app.on_key(key(KeyCode::Down), &vm, &src, &controls, now);
    app.on_key(key(KeyCode::Down), &vm, &src, &controls, now); // T178
    app.on_key(key(KeyCode::Enter), &vm, &src, &controls, now);
    app.on_key(key(KeyCode::End), &vm, &src, &controls, now);
    let t0 = Instant::now();
    let mut text = String::new();
    for _ in 0..20 {
        text = tui::buffer_text(&tui::render_to_buffer(&vm, &app, true, false, now, 200, 60));
    }
    let per = t0.elapsed() / 20;
    assert!(text.contains("T178-item-29999"), "{text}");
    assert!(text.contains("Items (30000 of 30000; state: all)"), "{text}");
    assert!(per < Duration::from_millis(250), "frame took {per:?}");
    app.on_key(key(KeyCode::Char('1')), &vm, &src, &controls, now); // filter open
    let t = tui::buffer_text(&tui::render_to_buffer(&vm, &app, true, false, now, 200, 60));
    assert!(t.contains("Items (7500 of 30000; state: open)"), "{t}");
}
