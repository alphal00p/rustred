//! Regression tests of the dashboard audit (WS/notes/implementer.audit-tui-A.md), one per finding id: each failed
//! before the fix (lane tuifix) and passes after. The pseudo-terminal halves of C1, M1 and M7 are in tui_pty.rs.

mod tui_support;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::style::Modifier;
use rustred_campaign::clock::UtcMillis;
use rustred_campaign::signals::StopLevel;
use rustred_campaign::tui::app::{Action, Panel};
use rustred_campaign::tui::clean::refused;
use rustred_campaign::tui::source::{ItemRow, RawSnapshot};
use rustred_campaign::tui::terminal::{Input, tui_loop};
use rustred_campaign::tui::theme::Theme;
use rustred_campaign::tui::{
    self, App, Controls, DirSource, MemorySource, ProgressSource, WatchControls, model, plain,
    reader::Reader,
};
use serde_json::{Value, json};
use tui_support::*;

const SIZES: [(u16, u16); 3] = [(80, 24), (120, 40), (200, 60)];

fn key(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

fn now_of(o: &tui::DashboardOptions) -> f64 {
    o.clock.now_utc().0 as f64 / 1000.0
}

fn mem(name: &str) -> MemorySource {
    MemorySource::new(demo::fixture_map(name))
}

fn has_refused(s: &str) -> Option<char> {
    s.chars().find(|c| refused(*c) && *c != '\n')
}

fn assert_buffer_clean(b: &ratatui::buffer::Buffer, what: &str) {
    let a = b.area;
    for y in a.y..a.bottom() {
        for x in a.x..a.right() {
            if let Some(c) = has_refused(b[(x, y)].symbol()) {
                panic!(
                    "{what}: refused character {c:?} in cell ({x},{y}):\n{}",
                    tui::buffer_text(b)
                );
            }
        }
    }
}

// ------------------------------------------------------------------------------------------------------------
// C1
// ------------------------------------------------------------------------------------------------------------

#[test]
fn c1_escape_injection_never_reaches_a_cell() {
    let src = demo::probe_source();
    let (o, _) = opts_at(fixture_now(&src.files["campaign.json"]));
    let vm = tui::view_model(&src, &o);
    assert_eq!(vm.jobs.list_origin, model::JobsOrigin::LedgerPartial);
    let now = now_of(&o);
    let w = WatchControls::new();
    let mut app = App::new(false, 120);
    let mut screens = 0;
    let mut check = |app: &App, what: &str| {
        for (wd, h) in SIZES.into_iter().chain([(79, 23)]) {
            let b = tui::render_to_buffer(&vm, app, true, true, now, wd, h);
            assert_buffer_clean(&b, &format!("{what} {wd}x{h}"));
            screens += 1;
        }
    };
    check(&app, "summary");
    for k in ['j', 'e', 'a', '?'] {
        app.on_key(key(KeyCode::Char(k)), &vm, &src, &w, now);
        check(&app, &format!("panel {k}"));
        app.on_key(key(KeyCode::Esc), &vm, &src, &w, now);
    }
    // every node panel (injected node id, phase; item rows with injected ids / labels / why; error strings)
    for i in 0..vm.nodes.len() {
        app.selected_node = i;
        app.on_key(key(KeyCode::Enter), &vm, &src, &w, now);
        assert_eq!(app.panel, Panel::Node);
        check(&app, &format!("node {i}"));
        app.on_key(key(KeyCode::Char('w')), &vm, &src, &w, now);
        check(&app, &format!("item of node {i}"));
        app.on_key(key(KeyCode::Esc), &vm, &src, &w, now);
        app.on_key(key(KeyCode::Esc), &vm, &src, &w, now);
    }
    assert!(screens > 40);
    // the injected text is visible as neutralised text (the placeholder), not dropped silently
    let t = tui::render_text(&vm, &App::new(false, 120), &o, 200, 60);
    assert!(t.contains("J\u{FFFD}]52;c;ZXZpbA==\u{FFFD}"), "{t}");
}

#[test]
fn c1_plain_once_and_json_outputs_are_sanitised() {
    let src = demo::probe_source();
    let (o, _) = opts_at(fixture_now(&src.files["campaign.json"]));
    let vm = tui::view_model(&src, &o);
    let app = App::new(false, 120);
    let mut texts = vec![plain::status_line(&vm, &app)];
    texts.extend(vm.events.iter().map(plain::event_line));
    for (w, h) in SIZES {
        texts.push(tui::render_once(&src, &o, w, h));
    }
    let json = tui::view_model_json_text(&src, &o);
    texts.push(json.clone());
    for t in &texts {
        assert_eq!(has_refused(t), None, "{t}");
    }
    let back: Value = serde_json::from_str(&json).expect("valid JSON");
    assert!(back["campaign_id"].as_str().unwrap().contains('\u{FFFD}'));
}

#[test]
fn c1_renderers_build_spans_only_from_clean_text() {
    // the renderers' only span constructors take `Clean`; no other tui file builds spans from text
    let render = include_str!("../src/tui/render.rs");
    let code: String = render
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(code.matches("Span::styled(").count(), 1, "only `styled`");
    assert_eq!(code.matches("Span::raw(").count(), 1, "only `gap`");
    for forbidden in [
        "Span::from(",
        "Line::raw(",
        "Line::styled(",
        "Line::from(\"",
        "Text::",
        "Paragraph::new(\"",
    ] {
        assert!(!code.contains(forbidden), "render.rs uses {forbidden}");
    }
    assert!(code.contains("fn styled(s: Clean, st: Style)"));
    for (name, src) in [
        ("model.rs", include_str!("../src/tui/model.rs")),
        ("plain.rs", include_str!("../src/tui/plain.rs")),
        ("app.rs", include_str!("../src/tui/app.rs")),
        ("terminal.rs", include_str!("../src/tui/terminal.rs")),
        ("mod.rs", include_str!("../src/tui/mod.rs")),
        ("source.rs", include_str!("../src/tui/source.rs")),
        ("reader.rs", include_str!("../src/tui/reader.rs")),
    ] {
        assert!(!src.contains("Span::"), "{name} builds spans");
        assert!(!src.contains("Line::from"), "{name} builds lines");
    }
}

// ------------------------------------------------------------------------------------------------------------
// M1
// ------------------------------------------------------------------------------------------------------------

/// Scripted keys: (time since start, event).
struct Script {
    t0: Instant,
    events: Vec<(Duration, Event)>,
}

impl Input for Script {
    fn poll(&mut self, timeout: Duration) -> std::io::Result<Option<Event>> {
        if let Some((at, _)) = self.events.first() {
            let due = self.t0 + *at;
            let now = Instant::now();
            if now >= due {
                return Ok(Some(self.events.remove(0).1));
            }
            std::thread::sleep(timeout.min(due - now));
            return Ok(None);
        }
        std::thread::sleep(timeout);
        Ok(None)
    }
}

struct Slow {
    inner: MemorySource,
    read: Duration,
    items: Duration,
}

impl ProgressSource for Slow {
    fn read(&self) -> RawSnapshot {
        std::thread::sleep(self.read);
        self.inner.read()
    }
    fn node_items(&self, n: &str) -> Result<Vec<ItemRow>, String> {
        std::thread::sleep(self.items);
        self.inner.node_items(n)
    }
}

fn run_script(
    src: Slow,
    events: Vec<(Duration, Event)>,
    controls: &dyn Controls,
) -> (tui::ExitReason, Duration, String) {
    let c = src.inner.files["campaign.json"].clone();
    let (o, _) = opts_at(fixture_now(&c));
    let reader = Reader::spawn(Arc::new(src), o.refresh);
    let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
    let mut input = Script {
        t0: Instant::now(),
        events,
    };
    let t0 = Instant::now();
    let (r, _, _) = tui_loop(
        &mut term,
        &mut input,
        &|| None,
        &reader,
        controls,
        &o,
        Theme { color: false },
        false,
        false,
    );
    let took = t0.elapsed();
    (r, took, tui::buffer_text(term.backend().buffer()))
}

#[test]
fn m1_slow_reads_do_not_delay_ctrl_c() {
    let src = Slow {
        inner: mem("midrun"),
        read: Duration::from_secs(5),
        items: Duration::ZERO,
    };
    let ctrl_c = Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    let (r, took, screen) = run_script(
        src,
        vec![(Duration::from_millis(300), ctrl_c)],
        &WatchControls::new(),
    );
    assert_eq!(r, tui::ExitReason::ViewerQuit);
    assert!(took < Duration::from_millis(1500), "took {took:?}");
    assert!(screen.contains("READING"), "{screen}");
}

/// A supervisor stand-in whose stop requests are counted; it reports done 1 s after the first one.
struct FakeSupervisor {
    count: std::sync::atomic::AtomicUsize,
    first: std::sync::Mutex<Option<Instant>>,
}

impl Controls for FakeSupervisor {
    fn request_stop(&self) -> StopLevel {
        self.first.lock().unwrap().get_or_insert_with(Instant::now);
        StopLevel::from_count(self.count.fetch_add(1, Ordering::SeqCst) + 1)
    }
    fn stop_count(&self) -> usize {
        self.count.load(Ordering::SeqCst)
    }
    fn hosts_campaign(&self) -> bool {
        true
    }
    fn campaign_done(&self) -> bool {
        self.first
            .lock()
            .unwrap()
            .is_some_and(|t| t.elapsed() > Duration::from_millis(700))
    }
}

#[test]
fn m1_ctrl_c_reaches_the_supervisor_while_a_drill_down_blocks() {
    let src = Slow {
        inner: mem("midrun"),
        read: Duration::ZERO,
        items: Duration::from_secs(6),
    };
    let sup = FakeSupervisor {
        count: Default::default(),
        first: Default::default(),
    };
    let ctrl_c = Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    let (r, took, screen) = run_script(
        src,
        vec![
            (Duration::from_millis(400), Event::Key(key(KeyCode::Enter))),
            (Duration::from_millis(700), ctrl_c),
        ],
        &sup,
    );
    assert_eq!(r, tui::ExitReason::StopRequested(StopLevel::Drain));
    assert_eq!(sup.stop_count(), 1);
    assert!(took < Duration::from_millis(2500), "took {took:?}");
    assert!(
        screen.contains("loading the item list") && screen.contains("STOPPING"),
        "{screen}"
    );
}

#[test]
fn m1_old_data_is_labelled_with_its_age() {
    let src = mem("midrun");
    let (o, _) = opts_at(fixture_now(&src.files["campaign.json"]));
    let mut vm = tui::view_model(&src, &o);
    vm.data_age_s = Some(15.0);
    let t = tui::render_text(&vm, &App::new(false, 120), &o, 120, 40);
    assert!(t.contains("data 15s old"), "{t}");
    assert!(t.contains("DATA 15s OLD"), "{t}");
    assert!(t.contains("in use (15s old)"), "{t}");
    assert!(
        t.contains("ETA ? (the progress files are slow to read"),
        "{t}"
    );
}

#[test]
fn m1_ledger_job_fallback_is_labelled_partial_and_skipped_when_files_list_jobs() {
    let mut src = mem("midrun");
    let (o, _) = opts_at(fixture_now(&src.files["campaign.json"]));
    src.jobs = Some(vec![]);
    // jobs.json lists the running jobs: the fallback is not used
    let vm = tui::view_model(&src, &o);
    assert_eq!(vm.jobs.list_origin, model::JobsOrigin::Progress);
    assert_eq!(vm.jobs.list.as_ref().unwrap().len(), 3);
    // a jobs.json with an empty list is a list (never replaced by the fallback)
    let saved = src.files["jobs.json"].clone();
    src.files.get_mut("jobs.json").unwrap()["list"] = json!([]);
    let vm = tui::view_model(&src, &o);
    assert_eq!(vm.jobs.list_origin, model::JobsOrigin::Progress);
    assert_eq!(vm.jobs.list.as_ref().unwrap().len(), 0);
    src.files.insert("jobs.json".into(), saved);
    src.files.remove("jobs.json");
    let vm = tui::view_model(&src, &o);
    assert_eq!(vm.jobs.list_origin, model::JobsOrigin::LedgerPartial);
    let t = tui::render_text(&vm, &App::new(false, 120), &o, 120, 40);
    assert!(
        t.contains("(from ledger, partial: no CPU / RSS / elapsed)"),
        "{t}"
    );
    // the DirSource fallback is bounded: a 0-byte cap switches it off
    let d = scratch("fallback");
    ledger_golden_cdir(&d);
    std::fs::create_dir_all(rustred_campaign::ledger::ledger_dir(&d)).unwrap();
    std::fs::write(
        rustred_campaign::ledger::ledger_dir(&d).join("segment-000001.jsonl"),
        b"x",
    )
    .unwrap();
    let mut ds = DirSource::new(&d);
    ds.job_fallback_cap_bytes = 0;
    assert!(ds.running_jobs().is_none());
    assert!(ds.job_fallback_off().unwrap().contains("bound"));
    let _ = std::fs::remove_dir_all(&d);
}

// ------------------------------------------------------------------------------------------------------------
// M2
// ------------------------------------------------------------------------------------------------------------

#[test]
fn m2_rules_column_never_shows_zero_beside_a_used_share() {
    let src = mem("midrun");
    let (o, _) = opts_at(fixture_now(&src.files["campaign.json"]));
    let vm = tui::view_model(&src, &o);
    let n = vm.nodes.iter().find(|n| n.id == "T266").unwrap();
    assert!(!n.economy_measured);
    assert_eq!(n.rules_in_use, Some(12658));
    let t = tui::render_text(&vm, &App::new(false, 120), &o, 120, 40);
    let row = t
        .lines()
        .find(|l| l.trim_start_matches('!').starts_with("T266 "))
        .unwrap();
    assert!(row.contains("12,658") && row.contains("37.6%"), "{row}");
    assert!(vm.suspect.is_empty() && vm.nodes.iter().all(|n| n.suspect.is_empty()));
}

#[test]
fn m2_contradicting_numbers_are_marked_and_listed() {
    let mut src = mem("midrun");
    let (o, _) = opts_at(fixture_now(&src.files["campaign.json"]));
    src.files.get_mut("campaign.json").unwrap()["certified"]["share"] = json!(0.991);
    src.files.get_mut("nodes/T162.json").unwrap()["closure"]["used_share"] = json!(0.9);
    let vm = tui::view_model(&src, &o);
    assert!(vm.suspect.contains("cert"));
    let n = vm.nodes.iter().find(|n| n.id == "T162").unwrap();
    assert!(n.suspect.contains("used"));
    assert!(vm.problems.iter().any(|p| {
        p.as_str()
            .starts_with("inconsistent: certified.share 99.1% but 987 of 1144")
    }));
    let t = tui::render_text(&vm, &App::new(false, 120), &o, 120, 40);
    assert!(t.contains("at the level asked: 1 of 4 nodes?"), "{t}");
    assert!(t.contains("90.0%?"), "{t}");
    assert!(t.contains("! inconsistent"), "{t}");
}

// ------------------------------------------------------------------------------------------------------------
// M3
// ------------------------------------------------------------------------------------------------------------

#[test]
fn m3_certification_at_the_level_each_node_asks() {
    let src = mem("midrun");
    let (o, _) = opts_at(fixture_now(&src.files["campaign.json"]));
    let vm = tui::view_model(&src, &o);
    assert_eq!((vm.cert.nodes_at_level, vm.cert.nodes), (1, 4));
    let t = tui::render_text(&vm, &App::new(false, 120), &o, 200, 60);
    assert!(
        t.contains("CERTIFIED at the level asked: 1 of 4 nodes"),
        "{t}"
    );
    assert!(
        t.contains("142 at replay") && t.contains("870 per-key") && t.contains("132 pending"),
        "{t}"
    );
    assert!(
        t.contains("replay/replay") && t.contains("per-key/-"),
        "{t}"
    );
    // a campaign whose nodes all reached their level never shows 0 %
    for name in ["toy", "finished"] {
        let src = mem(name);
        let (o, _) = opts_at(fixture_now(&src.files["campaign.json"]));
        let vm = tui::view_model(&src, &o);
        for (w, h) in SIZES {
            let t = tui::render_text(&vm, &App::new(false, 120), &o, w, h);
            let head: Vec<&str> = t.lines().take(10).collect();
            assert!(
                !head
                    .iter()
                    .any(|l| l.contains("CERT") && l.contains(" 0.0%")),
                "{name}: {t}"
            );
        }
    }
    let src = mem("finished");
    let (o, _) = opts_at(fixture_now(&src.files["campaign.json"]));
    let t = tui::render_once(&src, &o, 120, 40);
    assert!(
        t.contains("CERTIFIED at the level asked: 2 of 3 nodes"),
        "{t}"
    );
}

// ------------------------------------------------------------------------------------------------------------
// M4
// ------------------------------------------------------------------------------------------------------------

#[test]
fn m4_stale_values_are_dimmed_and_aged_and_the_banner_says_since_when() {
    let mut src = mem("stale");
    // without heartbeat.json (an older writer) staleness is inferred from file times, and labelled so
    src.files.remove("heartbeat.json");
    let (o, _) = opts_at(fixture_now(&src.files["campaign.json"]));
    let vm = tui::view_model(&src, &o);
    assert_eq!(vm.banner, model::Banner::Stale);
    assert_eq!(vm.heartbeat_source, model::HeartbeatSource::Inferred);
    let t = tui::render_text(&vm, &App::new(false, 120), &o, 120, 40);
    assert!(t.contains(" STALE since "), "{t}");
    assert!(
        t.contains("[no heartbeat field: staleness inferred from file times]"),
        "{t}"
    );
    assert!(t.contains("in use (10m06s old)"), "{t}");
    assert!(t.contains("queued (old)"), "{t}");
    assert!(
        t.contains("ETA ? (supervisor not running; last estimate"),
        "{t}"
    );
    let b = tui::render_to_buffer(&vm, &App::new(false, 120), true, false, now_of(&o), 120, 40);
    let line = (0..40)
        .find(|y| {
            (0..120)
                .map(|x| b[(x, *y)].symbol().to_string())
                .collect::<String>()
                .starts_with("CPU ")
        })
        .unwrap();
    assert!(
        b[(4, line)].modifier.contains(Modifier::DIM),
        "CPU value dimmed"
    );
}

#[test]
fn m4_supervisor_heartbeat_field_is_read_when_present() {
    let mut src = mem("stale");
    let c = src.files["campaign.json"].clone();
    let now = fixture_now(&c);
    let (o, _) = opts_at(now);
    let fresh = UtcMillis(now.0 - 3000).to_iso();
    src.files.insert(
        "heartbeat.json".into(),
        json!({"schema": "rustred.campaign.progress.v1", "file": "heartbeat",
               "supervisor": {"heartbeat_utc": fresh, "stopping": false}}),
    );
    let vm = tui::view_model(&src, &o);
    assert_eq!(vm.heartbeat_source, model::HeartbeatSource::Field);
    assert_eq!(
        vm.banner,
        model::Banner::Running,
        "heartbeat 3 s old: alive"
    );
    let old = UtcMillis(now.0 - 600_000).to_iso();
    src.files.get_mut("heartbeat.json").unwrap()["supervisor"]["heartbeat_utc"] = json!(old);
    let vm = tui::view_model(&src, &o);
    assert_eq!(vm.banner, model::Banner::Stale);
    let t = tui::render_text(&vm, &App::new(false, 120), &o, 200, 60);
    assert!(!t.contains("inferred from file times"), "{t}");
}

// ------------------------------------------------------------------------------------------------------------
// M5
// ------------------------------------------------------------------------------------------------------------

#[test]
fn m5_goldens_contain_no_machine_or_checkout_specific_text() {
    let dir = data().join("tui/screens");
    let mut n = 0;
    for e in std::fs::read_dir(&dir).unwrap().flatten() {
        let t = std::fs::read_to_string(e.path()).unwrap();
        assert_eq!(machine_specific(&t), None, "{}", e.path().display());
        n += 1;
    }
    assert!(n >= 27, "{n} goldens");
}

#[test]
fn m5_header_numbers_recompute_from_the_fixture_files() {
    for name in ["empty", "midrun", "finished", "stale", "toy"] {
        let dir = fixture_dir(name);
        let c = read_json(&dir.join("progress/campaign.json"));
        let nodes: Vec<Value> = std::fs::read_dir(dir.join("progress/nodes"))
            .map(|rd| {
                rd.flatten()
                    .map(|e| read_json(&e.path()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let u = |p: &str| c.pointer(p).and_then(Value::as_u64).unwrap_or(0);
        // the fixture's own numbers agree with each other
        assert_eq!(u("/nodes/total"), nodes.len() as u64, "{name}: nodes");
        let bc: u64 = c["nodes"]["by_closure"]
            .as_object()
            .unwrap()
            .values()
            .filter_map(Value::as_u64)
            .sum();
        assert_eq!(bc, nodes.len() as u64, "{name}: by_closure");
        let by_phase: u64 = c["items"]["by_phase"]
            .as_object()
            .unwrap()
            .values()
            .filter_map(Value::as_u64)
            .sum();
        assert_eq!(u("/items/total"), by_phase, "{name}: items");
        let (p, a) = (u("/certified/identity_proved"), u("/certified/accepted"));
        if let Some(sh) = c["certified"]["share"].as_f64() {
            assert!((sh - p as f64 / a as f64).abs() <= 0.0005, "{name}: share");
        }
        assert!(u("/closure/nodes_with_defect") <= u("/closure/nodes_checked"));
        assert!(u("/closure/nodes_checked") <= nodes.len() as u64);
        // ... and the screen shows exactly the recomputed numbers
        let src = fixture_source(name);
        let o = opts_for(&c);
        let vm = tui::view_model(&src, &o);
        assert!(vm.suspect.is_empty(), "{name}: {:?}", vm.problems);
        assert!(
            !vm.problems
                .iter()
                .any(|p| p.as_str().starts_with("inconsistent")),
            "{name}: {:?}",
            vm.problems
        );
        let at_level = nodes
            .iter()
            .filter(|n| {
                match (
                    n.pointer("/certification/asked"),
                    n.pointer("/certification/reached"),
                ) {
                    (Some(Value::String(x)), Some(Value::String(y))) => x == y,
                    (Some(Value::String(_)), _) => false,
                    _ => n["certified"].as_bool().unwrap_or(false),
                }
            })
            .count();
        for (w, h) in [(120u16, 40u16), (200, 60)] {
            let t = tui::render_once(&src, &o, w, h);
            assert!(t.contains(&format!("NODES {}", nodes.len())), "{name}: {t}");
            assert!(
                t.contains(&format!(
                    "CERTIFIED at the level asked: {at_level} of {} nodes",
                    nodes.len()
                )),
                "{name}: {t}"
            );
            let total = u("/items/total");
            if total > 0 {
                let open: u64 = c["items"]["by_phase"]
                    .as_object()
                    .unwrap()
                    .iter()
                    .filter(|(k, _)| model::phase_is_open(k))
                    .filter_map(|(_, v)| v.as_u64())
                    .sum();
                let done = total - open;
                let pct = format!(
                    "{:.0}% ({} of",
                    done as f64 * 100.0 / total as f64,
                    fmt(done)
                );
                assert!(t.contains(&pct), "{name}: {pct}\n{t}");
            }
        }
    }
}

fn fmt(n: u64) -> String {
    rustred_campaign::tui::format::thousands(n)
}

// ------------------------------------------------------------------------------------------------------------
// M6
// ------------------------------------------------------------------------------------------------------------

fn generic_campaign() -> MemorySource {
    let long = "sixloop-family-0042-with-a-rather-long-node-name";
    let mut m = BTreeMap::new();
    m.insert(
        "campaign.json".to_string(),
        json!({"schema": "rustred.campaign.progress.v1", "campaign_id": "six", "written_utc": "2030-01-01T00:00:00.000Z",
            "ledger": {"seq": 9}, "campaign": {"phase": "running"},
            "nodes": {"total": 2, "by_closure": {"full": 0, "boxed": 0, "open": 2}, "certified": 0},
            "items": {"total": 9, "by_phase": {"open": 9},
                "by_level": {"P10": {"open": 1}, "P2": {"open": 2}, "P0": {"open": 3}, "a/b": {"open": 3}}},
            "cost": {"by_level": {"a/b": {"jobs": 7, "p50_s": 3.0}, "P2": {"jobs": 5}}}}),
    );
    for (id, open) in [(long, 6u64), ("n2", 3)] {
        m.insert(
            format!("nodes/{id}.json"),
            json!({"schema": "rustred.campaign.progress.v1", "node": id, "phase": "running", "closure_kind": "open",
                "items": {"open": open, "by_phase": {"open": open},
                    "by_level": {"P10": {"open": 1}, "P2": {"open": 2}, "P0": {"open": open - 3}}},
                "closure": {"defect_keys": 5, "rules_used": 4, "rules_total": 8, "used_share": 0.5}}),
        );
    }
    MemorySource::new(m)
}

#[test]
fn m6_names_of_any_length_and_rungs_from_the_data() {
    let src = generic_campaign();
    let (o, _) = opts_at(UtcMillis::parse_iso("2030-01-01T00:00:02.000Z").unwrap());
    let vm = tui::view_model(&src, &o);
    let rungs: Vec<&str> = vm.ladder.iter().map(|l| l.rung.as_str()).collect();
    assert_eq!(
        rungs,
        ["P0", "P2", "P10", "a/b"],
        "natural order, no fixed list"
    );
    let ab = vm.ladder.iter().find(|l| l.rung == "a/b").unwrap();
    assert_eq!(
        ab.jobs,
        Some(7),
        "rung names with '/' keep their cost (no JSON pointer)"
    );
    for (w, h) in SIZES {
        let t = tui::render_text(&vm, &App::new(false, 120), &o, w, h);
        for l in t.lines() {
            assert!(l.chars().count() <= w as usize, "{w}: {l}");
        }
        // the long id keeps both ends (middle cut), in every width
        assert!(
            t.contains("sixloop-fam") && t.contains("node-name"),
            "{w}x{h}: {t}"
        );
        if w >= 120 {
            assert!(t.contains("P0:3 P2:2 P10:1"), "{w}x{h}: {t}");
        }
    }
    // the node detail (wide column / node panel) shows the full id
    let mut app = App::new(false, 120);
    app.selected_node = vm
        .nodes
        .iter()
        .position(|n| n.id.as_str().starts_with("six"))
        .unwrap();
    let t = tui::render_text(&vm, &app, &o, 200, 60);
    assert!(
        t.contains("sixloop-family-0042-with-a-rather-long-node-name"),
        "{t}"
    );
}

#[test]
fn m6_no_essential_cell_is_cut_at_200_and_120_columns() {
    let src = mem("midrun");
    let (o, _) = opts_at(fixture_now(&src.files["campaign.json"]));
    let vm = tui::view_model(&src, &o);
    for (w, h) in [(200u16, 60u16), (120, 40)] {
        let t = tui::render_text(&vm, &App::new(false, 120), &o, w, h);
        for n in &vm.nodes {
            let by: Vec<String> = n
                .open_by_rung
                .iter()
                .map(|(r, k)| format!("{r}:{k}"))
                .collect();
            if !by.is_empty() {
                assert!(t.contains(&by.join(" ")), "{w}: {} {by:?}\n{t}", n.id);
            }
        }
        let nodes_rows: Vec<&str> = t
            .lines()
            .skip_while(|l| !l.starts_with("-[ Nodes"))
            .skip(1)
            .take(5)
            .collect();
        assert!(
            nodes_rows.iter().all(|l| !l.contains('~')),
            "{w}: {nodes_rows:?}"
        );
    }
    // at 80 columns the least important columns go, the identity and the state stay
    let t = tui::render_text(&vm, &App::new(false, 120), &o, 80, 24);
    let head = t
        .lines()
        .find(|l| l.starts_with("node"))
        .expect("nodes table at 80 columns");
    for c in ["node", "coverage", "open", "defect"] {
        assert!(head.contains(c), "{head}");
    }
    assert!(!head.contains("CPU-h") && !head.contains("RHS"), "{head}");
}

// ------------------------------------------------------------------------------------------------------------
// minor findings
// ------------------------------------------------------------------------------------------------------------

#[test]
fn minor_m1_eta_wording_finishing_time_and_cpu_hours() {
    let src = mem("empty");
    let (o, _) = opts_at(fixture_now(&src.files["campaign.json"]));
    let t = tui::render_once(&src, &o, 120, 40);
    assert!(t.contains("ETA ? (no work registered yet)"), "{t}");
    let src = mem("midrun");
    let (o, _) = opts_at(fixture_now(&src.files["campaign.json"]));
    let t = tui::render_once(&src, &o, 200, 60);
    assert!(t.contains("ETA ~ 54m-2.6h"), "{t}");
    assert!(t.contains("done ~ 09:09-10:51 UTC"), "{t}");
    assert!(
        t.contains("(18-55 CPU-h left; 186 items queued or running, 21 jobs)"),
        "{t}"
    );
    assert!(t.contains("escalations not counted"), "{t}");
}

#[test]
fn minor_m2_shares_come_from_their_denominators() {
    let c = json!({"schema":"rustred.campaign.progress.v1","ledger":{"seq":2},
        "written_utc":"2026-11-20T08:15:04.000Z","campaign":{"phase":"starting"},
        "certified":{"share":1.0,"identity_proved":0,"accepted":0}});
    let mut m = BTreeMap::new();
    m.insert("campaign.json".to_string(), c.clone());
    let src = MemorySource::new(m);
    let o = opts_for(&c);
    let vm = tui::view_model(&src, &o);
    assert_eq!(vm.cert.per_key_share(), None);
    assert!(vm.suspect.contains("cert"));
    let t = tui::render_once(&src, &o, 120, 40);
    let cert = t.lines().find(|l| l.starts_with("CERTIFIED")).unwrap();
    assert!(!cert.contains("100"), "{t}");
}

#[test]
fn minor_m8_third_ctrl_c_needs_a_pause() {
    let src = mem("midrun");
    let (o, _) = opts_at(fixture_now(&src.files["campaign.json"]));
    let vm = tui::view_model(&src, &o);
    let sup = FakeSupervisor {
        count: Default::default(),
        first: Default::default(),
    };
    let mut app = App::new(true, 120);
    let cc = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert_eq!(app.on_key(cc, &vm, &src, &sup, 0.0), Action::Redraw);
    assert_eq!(app.on_key(cc, &vm, &src, &sup, 0.1), Action::Redraw);
    assert_eq!(
        app.on_key(cc, &vm, &src, &sup, 0.2),
        Action::Redraw,
        "burst"
    );
    assert_eq!(sup.stop_count(), 2);
    assert_eq!(
        app.on_key(cc, &vm, &src, &sup, 1.5),
        Action::Quit(tui::ExitReason::ForcedExit)
    );
}

#[test]
fn minor_m9_heartbeat_stamps_in_the_future_are_ignored() {
    let now = UtcMillis::parse_iso("2026-11-20T08:15:00.000Z").unwrap();
    let c = json!({"schema":"rustred.campaign.progress.v1","ledger":{"seq":2},
        "written_utc":"2026-11-20T09:15:00.000Z","campaign":{"phase":"running"},
        "resources":{"cpus_allowed":4,"sampled_utc":"2026-11-20T08:05:00.000Z"}});
    let mut m = BTreeMap::new();
    m.insert("campaign.json".to_string(), c);
    let (o, _) = opts_at(now);
    let vm = tui::view_model(&MemorySource::new(m), &o);
    assert_eq!(vm.banner, model::Banner::Stale, "{:?}", vm.problems);
    assert!(
        vm.problems
            .iter()
            .any(|p| p.as_str().starts_with("clock skew"))
    );
}

#[test]
fn minor_m10_a_fifo_in_place_of_a_progress_file_does_not_block() {
    let d = scratch("fifo");
    std::fs::create_dir_all(d.join("progress")).unwrap();
    let p = d.join("progress/campaign.json");
    let cp = std::ffi::CString::new(p.as_os_str().as_encoded_bytes()).unwrap();
    // SAFETY: valid NUL-terminated path; creates a FIFO in our own scratch directory.
    assert_eq!(unsafe { libc::mkfifo(cp.as_ptr(), 0o600) }, 0);
    let done = Arc::new(AtomicBool::new(false));
    let d2 = d.clone();
    let done2 = Arc::clone(&done);
    let h = std::thread::spawn(move || {
        let snap = DirSource::new(&d2).read();
        done2.store(true, Ordering::SeqCst);
        snap
    });
    let t0 = Instant::now();
    while !done.load(Ordering::SeqCst) && t0.elapsed() < Duration::from_secs(3) {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(done.load(Ordering::SeqCst), "reading a FIFO blocked");
    let snap = h.join().unwrap();
    assert!(
        matches!(&snap.campaign, rustred_campaign::tui::source::FileRead::Invalid(w) if w.contains("not a regular file")),
        "{:?}",
        snap.campaign
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn minor_m11_wide_characters_keep_the_columns_aligned() {
    let mut src = generic_campaign();
    let v = src.files.remove("nodes/n2.json").unwrap();
    let mut v2 = v.clone();
    v2["node"] = json!("節点α");
    src.files.insert("nodes/節点α.json".into(), v2);
    let (o, _) = opts_at(UtcMillis::parse_iso("2030-01-01T00:00:02.000Z").unwrap());
    let vm = tui::view_model(&src, &o);
    let b = tui::render_to_buffer(
        &vm,
        &App::new(false, 120),
        false,
        false,
        now_of(&o),
        120,
        40,
    );
    // the cell column where the coverage word starts, in each node row (cells, as the terminal places them)
    let mut cols = Vec::new();
    for y in 0..40u16 {
        let sym: Vec<String> = (0..120u16)
            .map(|x| b[(x, y)].symbol().to_string())
            .collect();
        let row: String = sym.concat();
        if !(row.starts_with("sixloop") || row.starts_with("節")) {
            continue;
        }
        let x = (0..116)
            .find(|&x| sym[x] == "O" && sym[x + 1] == "P" && sym[x + 2] == "E" && sym[x + 3] == "N")
            .expect("coverage word");
        cols.push(x);
    }
    assert_eq!(cols.len(), 2, "two node rows");
    assert_eq!(cols[0], cols[1], "columns aligned");
}
