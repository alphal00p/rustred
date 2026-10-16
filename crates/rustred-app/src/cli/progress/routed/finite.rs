//! Finite scope progress comes only from the last native committed checkpoint.
//! Keep fixed-size summary fields in the existing latest-event slot; no cursor
//! scans or additional locks on observation or heartbeat.
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Cell, Gauge, Padding, Paragraph, Row, Table, Wrap},
};
use serde_json::{Value, json};

pub(in crate::cli::progress) const HEIGHT: u16 = 16;

fn number(value: &Value) -> String {
    value.as_u64().map_or_else(
        || "—".into(),
        |n| {
            let digits = n.to_string();
            let mut result = String::new();
            for (index, digit) in digits.chars().enumerate() {
                if index > 0 && (digits.len() - index).is_multiple_of(3) {
                    result.push(',');
                }
                result.push(digit);
            }
            result
        },
    )
}

fn duration(seconds: f64) -> String {
    let seconds = seconds.max(0.) as u64;
    format!(
        "{:02}:{:02}:{:02}",
        seconds / 3600,
        (seconds / 60) % 60,
        seconds % 60
    )
}

fn checkpoint_note(event: &Value) -> &'static str {
    if event["finite_committed"].is_object() && event["finite_generation"].is_object() {
        "checkpoint retained"
    } else {
        "no checkpoint reported"
    }
}

fn stage(event: &Value) -> String {
    let status = event["result"]["status"]
        .as_str()
        .or(event["status"].as_str());
    let current = match status {
        Some("complete" | "completed" | "limited" | "cancelled" | "paused" | "failed") => status,
        _ => event["phase"]
            .as_str()
            .or(status)
            .or(event["event"].as_str()),
    };
    let label: String = match current.unwrap_or("preparing") {
        "starting" | "preparing" => "Preparing immutable source".into(),
        "finite_backend_native" => "Solving current window".into(),
        "finite_checkpoint_saved" => "Checkpoint committed".into(),
        "finite_checkpoint_restored" => "Checkpoint restored".into(),
        "finite_backend_resident_hydrated" | "active" => "Work cache ready".into(),
        "finite_backend_resident_fallback" | "declined" => "Continuing with paged Work".into(),
        "complete" | "completed" => "Required scope complete".into(),
        "limited" => format!("Resource limit reached · {}", checkpoint_note(event)),
        "cancelled" | "paused" => format!("Paused · {}", checkpoint_note(event)),
        "failed" => format!("Stopped · {}", checkpoint_note(event)),
        other => other
            .strip_prefix("finite_backend_")
            .unwrap_or(other)
            .replace('_', " "),
    };
    match preparation_counts(event) {
        Some((done, total)) => format!("{label} · preparation {done}/{total}"),
        None => label,
    }
}

fn preparation_counts(event: &Value) -> Option<(u64, u64)> {
    event["phase"].as_str()?;
    Some((event["completed"].as_u64()?, event["total"].as_u64()?))
}

pub(super) fn window_identity(event: &Value) -> Option<(u64, u64)> {
    let local = &event["finite_local"];
    if !local["snapshot"].is_object() {
        return None;
    }
    Some((local["row"].as_u64()?, local["window"].as_u64()?))
}

fn cpu_color(cores: Option<f64>, workers: Option<u64>) -> Color {
    match cores.zip(workers).filter(|(_, workers)| *workers > 0) {
        Some((cores, workers)) if cores / (workers as f64) < 0.5 => Color::Red,
        Some((cores, workers)) if cores / (workers as f64) < 0.75 => Color::Yellow,
        Some(_) => Color::Green,
        None => Color::Gray,
    }
}

pub(super) fn plain_status(record: &Value) -> String {
    let event = &record["progress"];
    let committed = &event["finite_committed"];
    let cpu = record["process_cpu_cores"]
        .as_f64()
        .map_or_else(|| "—".into(), |n| format!("{n:.2}"));
    let error = event["result"]["error"]
        .as_str()
        .map_or_else(String::new, |detail| {
            format!(
                " | {}",
                detail
                    .chars()
                    .filter(|c| !c.is_control())
                    .take(200)
                    .collect::<String>()
            )
        });
    format!(
        "RustRed finite | {} | committed {}/{} rows | generation {} | CPU {}/{} cores | elapsed {}{error}",
        stage(event),
        number(&committed["exhausted_required_rows"]),
        number(&committed["required_rows"]),
        number(&event["finite_generation"]["number"]),
        cpu,
        number(&record["reserved_workers"]),
        duration(record["elapsed_seconds"].as_f64().unwrap_or(0.))
    )
}

/// One bounded presentation of native counters. It never derives closure from
/// transient node counts, and ratatui clips every cell to the current viewport.
pub(in crate::cli::progress) fn render(frame: &mut Frame<'_>, record: &Value, color: bool) {
    let event = &record["progress"];
    let committed = &event["finite_committed"];
    let local = &event["finite_local"];
    let snapshot = &local["snapshot"];
    let failure = event["result"]["error"]
        .as_str()
        .or(snapshot["first_failure"]["detail"].as_str());
    let stopped = failure.is_some() || record["cancel_requested"] == true;
    let complete = committed["required_complete"] == true;
    let tone = if stopped {
        Color::Yellow
    } else if complete {
        Color::Green
    } else {
        Color::Cyan
    };
    let paint = |tone| {
        if color {
            Style::default().fg(tone)
        } else {
            Style::default()
        }
    };
    let title = if color {
        paint(tone).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(paint(tone))
        .title(Line::from(vec![
            Span::styled(" RustRed ", title),
            Span::styled("│ FINITE DEMAND ", paint(Color::Gray)),
        ]))
        .padding(Padding::horizontal(1));
    let inner = block.inner(frame.area());
    frame.render_widget(block, frame.area());
    let sections = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(8),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .split(inner);
    frame.render_widget(Paragraph::new(stage(event)).style(title), sections[0]);
    let done = committed["exhausted_required_rows"].as_u64();
    let total = committed["required_rows"].as_u64();
    let ratio = match (done, total) {
        (Some(done), Some(total)) if total > 0 => (done.min(total) as f64 / total as f64).min(1.),
        _ => {
            if complete {
                1.
            } else {
                0.
            }
        }
    };
    let label = match (done, total) {
        (Some(_), Some(_)) => format!(
            "{} / {} required rows closed  ·  {:.1}%",
            number(&committed["exhausted_required_rows"]),
            number(&committed["required_rows"]),
            ratio * 100.
        ),
        _ => "Waiting for first committed checkpoint".into(),
    };
    frame.render_widget(
        Gauge::default()
            .ratio(ratio)
            .label(label)
            .gauge_style(paint(if stopped { Color::Yellow } else { Color::Green })),
        sections[1],
    );
    frame.render_widget(
        Paragraph::new("CHECKPOINT · WINDOW · RESOURCES").style(paint(Color::DarkGray)),
        sections[2],
    );
    let position = |field| {
        local.get(field).unwrap_or(
            &committed[match field {
                "row" => "current_row",
                _ => "current_window",
            }],
        )
    };
    let elapsed = duration(record["elapsed_seconds"].as_f64().unwrap_or(0.));
    let memory = record["process_rss_bytes"].as_u64().map_or_else(
        || "—".into(),
        |n| format!("{:.2} GiB", n as f64 / (1u64 << 30) as f64),
    );
    let cpu = record["process_cpu_cores"].as_f64();
    let reserved = record["reserved_workers"].as_u64();
    let cpu_tone = cpu_color(cpu, reserved);
    let cpu_usage = cpu.zip(reserved).filter(|(_, workers)| *workers > 0);
    let first = if preparation_counts(event).is_some() {
        (
            "Stage completed",
            number(&event["completed"]),
            "Stage total",
            number(&event["total"]),
        )
    } else {
        (
            "Saved Work",
            number(&committed["completed_work"]),
            "Input occurrences",
            number(&committed["input_occurrences"]),
        )
    };
    let metrics = [
        first,
        (
            "Generation",
            number(&event["finite_generation"]["number"]),
            "Auxiliary closed",
            format!(
                "{} / {}",
                number(&committed["exhausted_auxiliary_rows"]),
                number(&committed["auxiliary_rows"])
            ),
        ),
        (
            "Row (0-based)",
            number(position("row")),
            "Window (0-based)",
            number(position("window")),
        ),
        (
            "Expanded nodes",
            number(&snapshot["completed_nodes"]),
            "Discovered nodes",
            number(&snapshot["scheduled_nodes"]),
        ),
        (
            "Queued nodes",
            number(&snapshot["queued_nodes"]),
            "Native busy",
            format!(
                "{} / {}",
                number(&snapshot["active_nodes"]),
                number(&snapshot["workers"])
            ),
        ),
        (
            "CPU / reserved",
            format!(
                "{} / {}",
                cpu.map_or_else(|| "—".into(), |n| format!("{n:.2}")),
                number(&record["reserved_workers"])
            ),
            "Use of budget",
            cpu_usage.map_or_else(
                || "—".into(),
                |(cores, workers)| format!("{:.1}%", cores / workers as f64 * 100.),
            ),
        ),
        ("Elapsed", elapsed, "Process RSS", memory),
        (
            "Recent nodes/s",
            record["recent_nodes_per_second"]
                .as_f64()
                .map_or_else(|| "—".into(), |n| format!("{n:.1}")),
            "Last update",
            format!(
                "{:.1}s ago",
                record["progress_age_seconds"].as_f64().unwrap_or(0.)
            ),
        ),
    ];
    let wide = sections[3].width >= 88;
    let value = |text: String, tone| {
        Cell::from(Text::from(text).alignment(Alignment::Right)).style(paint(tone))
    };
    let rows = metrics.into_iter().map(|(left, lv, right, rv)| {
        let tone = if left == "CPU / reserved" {
            cpu_tone
        } else {
            Color::White
        };
        if wide {
            Row::new(vec![
                Cell::from(left).style(paint(Color::Gray)),
                value(lv, tone),
                Cell::from(right).style(paint(Color::Gray)),
                value(rv, tone),
            ])
        } else {
            let label = match left {
                "Saved Work" => "Work / inputs",
                "Stage completed" => "Stage done/total",
                "Generation" => "Gen / auxiliary",
                "Row (0-based)" => "Row / window (0)",
                "Expanded nodes" => "Expanded / seen",
                "Queued nodes" => "Queue / busy",
                "CPU / reserved" => "CPU / reserved",
                "Elapsed" => "Elapsed / RSS",
                _ => "Rate / age",
            };
            Row::new(vec![
                Cell::from(label).style(paint(Color::Gray)),
                value(format!("{lv}  │  {rv}"), tone),
            ])
        }
    });
    let widths = if wide {
        vec![
            Constraint::Length(17),
            Constraint::Fill(1),
            Constraint::Length(17),
            Constraint::Fill(1),
        ]
    } else {
        vec![
            Constraint::Length(if sections[3].width < 40 { 10 } else { 16 }),
            Constraint::Fill(1),
        ]
    };
    frame.render_widget(Table::new(rows, widths).column_spacing(2), sections[3]);
    let residency = match event["finite_residency"]["status"].as_str() {
        Some("active") => "Work cache: resident",
        Some("disabled") => "Work cache: paged · resident cache disabled",
        Some("declined") => "Work cache: paged · resident grant unavailable",
        _ => "Work cache: preparing",
    };
    frame.render_widget(
        Paragraph::new(residency).style(paint(Color::Blue)),
        sections[4],
    );
    let footer = failure.map_or_else(
        || "Window nodes are local; required-row progress is committed. Native busy counts tasks, not CPU.".to_string(),
        |detail| format!("{} · {}", detail.chars().filter(|c| !c.is_control()).collect::<String>(), checkpoint_note(event)),
    );
    frame.render_widget(
        Paragraph::new(footer)
            .style(paint(if failure.is_some() {
                Color::Red
            } else {
                Color::DarkGray
            }))
            .wrap(Wrap { trim: true }),
        sections[5],
    );
}

pub(super) fn retain(previous: &Value, event: &mut Value) {
    if event["operation"] != "finite_demand" {
        return;
    }
    if previous["operation"] == "finite_demand" {
        for field in [
            "finite_committed",
            "finite_generation",
            "finite_residency",
            "finite_local",
        ] {
            if let Some(value) = previous.get(field) {
                event[field] = value.clone();
            }
        }
    }
    match event["event"].as_str() {
        Some("finite_checkpoint_saved" | "finite_checkpoint_restored") => {
            // These counters were computed at durable boundaries by native
            // lifecycle code. Native scheduled/completed nodes cannot advance them.
            event["finite_committed"] = event["committed"].clone();
            event["finite_generation"] = json!({
                "number":event["generation_number"], "digest":event["generation"]});
            // The next cursor may designate a new window. Never display the
            // preceding window's local completion as that window's progress.
            event["finite_local"] = Value::Null;
        }
        Some("finite_backend_native") => {
            event["finite_local"] = json!({"row":event["row"],"window":event["window"],
                "snapshot":event.get("snapshot").unwrap_or(&event["native"])});
        }
        Some("finite_backend_resident_hydrated" | "finite_backend_resident_fallback") => {
            event["finite_residency"] = json!({"status":event["status"],
                "requested_bytes":event["requested_bytes"],"resident":event["resident"],
                "fallback":event["event"] == "finite_backend_resident_fallback"});
        }
        Some("finished") => {
            // A resume can fail during preparation before emitting a restore
            // event. The final result may still contain an authenticated cold
            // inspection; use it only where no native observation was retained.
            for (field, result_field) in [
                ("finite_committed", "committed"),
                ("finite_generation", "generation"),
            ] {
                if !event[field].is_object() && event["result"][result_field].is_object() {
                    event[field] = event["result"][result_field].clone();
                }
            }
            event["finite_local"] = Value::Null;
        }
        _ => {}
    }
}

pub(super) fn local_snapshot(event: &Value) -> Option<&Value> {
    if event["operation"] == "finite_demand" {
        Some(&event["finite_local"]["snapshot"])
    } else {
        None
    }
}

pub(super) fn annotate_heartbeat(record: &mut Value) {
    if record["progress"]["operation"] != "finite_demand" {
        return;
    }
    let committed = &record["progress"]["finite_committed"];
    record["required_scope_progress"] = if committed.is_object() {
        json!({"closed_rows":committed["exhausted_required_rows"],
            "total_rows":committed["required_rows"],
            "required_complete":committed["required_complete"],
            "from_native_committed_checkpoint":true})
    } else {
        Value::Null
    };
    record["expanded_nodes_scope"] = json!("current_window_only_not_input_closure");
}

pub(super) fn dashboard(record: &Value) -> [String; 6] {
    let event = &record["progress"];
    let committed = &event["finite_committed"];
    let local = &event["finite_local"];
    let snapshot = &local["snapshot"];
    let n = |key| snapshot[key].as_u64().unwrap_or(0);
    let status = event["result"]["status"]
        .as_str()
        .or(event["status"].as_str())
        .or(event["event"].as_str())
        .unwrap_or("preparing");
    let required_line = match (
        committed["exhausted_required_rows"].as_u64(),
        committed["required_rows"].as_u64(),
    ) {
        (Some(done), Some(total)) => {
            let filled = if total == 0 {
                usize::from(committed["required_complete"] == true) * 20
            } else {
                ((done.min(total) as u128 * 20) / total as u128) as usize
            };
            format!(
                "[{}{}] {done}/{total} required rows closed in committed scope",
                "#".repeat(filled),
                "-".repeat(20 - filled)
            )
        }
        _ => "[--------------------] Required scope awaiting native checkpoint".into(),
    };
    let index = |value: &Value| value.as_u64().map_or_else(|| "—".into(), |n| n.to_string());
    let row = local.get("row").unwrap_or(&committed["current_row"]);
    let window = local.get("window").unwrap_or(&committed["current_window"]);
    let generation = index(&event["finite_generation"]["number"]);
    let residency = match event["finite_residency"]["status"].as_str() {
        Some("active") => "resident Work active",
        Some("disabled") => "paged Work (resident disabled)",
        Some("declined") if event["finite_residency"]["fallback"] == true => {
            "paged Work (resident grant exhausted)"
        }
        Some("declined") => "paged Work (resident grant declined)",
        _ => "Work residency pending",
    };
    let failure = snapshot["first_failure"]["detail"]
        .as_str()
        .or(event["result"]["error"].as_str());
    let last = if let Some(detail) = failure {
        let detail: String = detail
            .chars()
            .filter(|c| !c.is_control())
            .take(180)
            .collect();
        format!("Failure: {detail}; {}", checkpoint_note(event))
    } else {
        format!(
            "Last update {:.1}s ago; local nodes are not completed inputs; no family closure claim",
            record["progress_age_seconds"].as_f64().unwrap_or(0.)
        )
    };
    [
        format!("RustRed finite-demand campaign — {status}"),
        required_line,
        format!(
            "Row {row} / window {window} (zero-based): local nodes {}/{} expanded/discovered; {} queued",
            n("completed_nodes"),
            n("scheduled_nodes"),
            n("queued_nodes"),
            row = index(row),
            window = index(window)
        ),
        format!(
            "Committed Work {}  input occurrences {}  generation {generation}\nAuxiliary rows {}/{} closed; required complete {}\n{residency}",
            index(&committed["completed_work"]),
            index(&committed["input_occurrences"]),
            index(&committed["exhausted_auxiliary_rows"]),
            index(&committed["auxiliary_rows"]),
            committed["required_complete"]
                .as_bool()
                .map_or("unknown", |value| if value { "yes" } else { "no" })
        ),
        format!(
            "Native {}/{} active (not CPU utilization)  elapsed {:.1}s  RSS {:.3} GB  cancel {}",
            n("active_nodes"),
            n("workers"),
            record["elapsed_seconds"].as_f64().unwrap_or(0.),
            record["process_rss_bytes"].as_u64().unwrap_or(0) as f64 / 1e9,
            record["cancel_requested"]
        ),
        last,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn finite_table_resizes_and_keeps_committed_progress_distinct_from_window_counts() {
        let mut latest = json!({});
        observe(&mut latest, checkpoint(4_129, 3));
        observe(
            &mut latest,
            json!({"operation":"finite_demand", "event":"finite_backend_native",
            "row":3,"window":17,"snapshot":{"completed_nodes":12000,"scheduled_nodes":24000,
            "queued_nodes":8000,"active_nodes":4,"workers":8,"rule_applications":999,"transport_calls":11}}),
        );
        let record = json!({"progress":latest,"elapsed_seconds":3723.0,"process_rss_bytes":8u64 << 30,
            "process_cpu_cores":6.5,"reserved_workers":8,
            "recent_nodes_per_second":125.5,"progress_age_seconds":0.2});
        for (width, height) in [(120, HEIGHT), (80, HEIGHT), (40, HEIGHT), (10, 8), (1, 1)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal
                .draw(|frame| render(frame, &record, false))
                .unwrap();
            let buffer = terminal.backend().buffer();
            assert!(
                buffer
                    .content
                    .iter()
                    .all(|cell| cell.fg == Color::Reset && cell.bg == Color::Reset)
            );
            let text = buffer
                .content
                .chunks(width as usize)
                .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
                .collect::<Vec<_>>()
                .join("\n");
            if width == 120 {
                if let Some(path) = std::env::var_os("RUSTRED_FINITE_DASHBOARD_PREVIEW") {
                    std::fs::write(path, format!("{text}\n")).unwrap();
                }
                assert!(
                    text.contains("Recent nodes/s") && text.contains("Last update"),
                    "{text}"
                );
            }
            if width >= 80 {
                assert!(text.contains("3 / 10 required rows closed"), "{text}");
                assert!(text.contains("12,000"), "{text}");
                assert!(text.contains("24,000"), "{text}");
                assert!(text.contains("4,129"), "{text}");
                assert!(text.contains("01:02:03"), "{text}");
                assert!(text.contains("6.50 / 8"), "{text}");
                assert!(text.contains("81.2%"), "{text}");
                assert!(
                    text.contains("125.5") && text.contains("0.2s ago"),
                    "{text}"
                );
                assert!(text.contains("╭") && text.contains("╰"), "{text}");
            }
        }
    }

    #[test]
    fn finite_cpu_budget_thresholds_and_preparation_phase_are_explicit() {
        for (cores, expected) in [
            (3.99, Color::Red),
            (4.0, Color::Yellow),
            (5.99, Color::Yellow),
            (6.0, Color::Green),
            (10.0, Color::Green),
        ] {
            assert_eq!(cpu_color(Some(cores), Some(8)), expected);
        }
        assert_eq!(cpu_color(None, Some(8)), Color::Gray);
        assert_eq!(cpu_color(Some(1.), Some(0)), Color::Gray);
        let event = json!({"operation":"finite_demand","event":"preparation_progress","status":"preparing","phase":"owner_load"});
        assert_eq!(stage(&event), "owner load");
        let mut preparing = event.clone();
        preparing["completed"] = json!(17);
        preparing["total"] = json!(26);
        assert_eq!(stage(&preparing), "owner load · preparation 17/26");
        let mut terminal = Terminal::new(TestBackend::new(120, HEIGHT)).unwrap();
        terminal
            .draw(|frame| render(frame, &json!({"progress":preparing}), false))
            .unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(text.contains("Stage completed") && text.contains("Stage total"));
        assert!(text.contains("Waiting for first committed checkpoint"));
        assert!(!text.contains("17 / 26 required rows closed"));
        let record = json!({"progress":event,"process_cpu_cores":3.5,"reserved_workers":8});
        let plain = plain_status(&record);
        assert!(plain.contains("owner load") && plain.contains("CPU 3.50/8 cores"));
        assert!(!plain.contains('\u{1b}'));
    }

    #[test]
    fn finite_table_failure_is_colored_and_terminal_controls_are_not_replayed() {
        let mut latest = json!({});
        observe(&mut latest, checkpoint(5, 3));
        observe(
            &mut latest,
            json!({"operation":"finite_demand", "event":"finished",
            "result":{"status":"limited","error":"fan-out allowance\u{1b}[2J\ncheckpoint retained"}}),
        );
        let mut terminal = Terminal::new(TestBackend::new(120, HEIGHT)).unwrap();
        terminal
            .draw(|frame| render(frame, &json!({"progress":latest}), true))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let text = buffer
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(text.contains("Resource limit reached"));
        assert!(text.contains("3 / 10 required rows closed"));
        assert!(text.contains("fan-out allowance"));
        assert!(!text.contains('\u{1b}'));
        assert!(buffer.content.iter().any(|cell| cell.fg == Color::Red));
        assert!(buffer.content.iter().any(|cell| cell.fg == Color::Yellow));
    }

    fn checkpoint(generation: u64, done: u64) -> Value {
        json!({"operation":"finite_demand","event":"finite_checkpoint_saved",
            "generation_number":generation,"generation":"digest",
            "committed":{"required_rows":10,"exhausted_required_rows":done,
                "required_complete":false,"auxiliary_rows":2,"exhausted_auxiliary_rows":1,
                "current_row":done,"current_window":0,"completed_work":generation*100,
                "input_occurrences":generation*3}})
    }
    fn observe(previous: &mut Value, mut event: Value) {
        retain(previous, &mut event);
        *previous = event;
    }

    #[test]
    fn final_inspection_backfills_early_resume_failure_without_replacing_observed_checkpoint() {
        let finished = json!({"operation":"finite_demand", "event":"finished",
            "result":{"status":"failed","error":"owner load failed",
                "committed":{"required_rows":10,"exhausted_required_rows":7},
                "generation":{"number":9,"digest":"inspected"}}});
        let mut latest = json!({});
        observe(&mut latest, finished.clone());
        assert_eq!(latest["finite_committed"]["exhausted_required_rows"], 7);
        assert_eq!(latest["finite_generation"]["number"], 9);
        assert_eq!(checkpoint_note(&latest), "checkpoint retained");
        let mut latest = json!({});
        observe(&mut latest, checkpoint(5, 3));
        observe(&mut latest, finished);
        assert_eq!(latest["finite_committed"]["exhausted_required_rows"], 3);
        assert_eq!(latest["finite_generation"]["number"], 5);
        let mut fresh = json!({});
        observe(
            &mut fresh,
            json!({"operation":"finite_demand", "event":"finished",
            "result":{"status":"failed","error":"initial preparation failed"}}),
        );
        assert_eq!(checkpoint_note(&fresh), "no checkpoint reported");
        assert!(stage(&fresh).contains("no checkpoint reported"));
        assert!(!dashboard(&json!({"progress":fresh}))[5].contains("checkpoint retained"));
    }

    #[test]
    fn finite_committed_scope_survives_native_windows_without_claiming_input_completion() {
        let mut latest = json!({});
        observe(&mut latest, checkpoint(2, 1));
        for window in [0, 1] {
            observe(
                &mut latest,
                json!({"operation":"finite_demand","event":"finite_backend_native",
                "row":1,"window":window,"snapshot":{"scheduled_nodes":100,"completed_nodes":100,
                    "active_nodes":0,"workers":4}}),
            );
            assert_eq!(latest["finite_committed"]["exhausted_required_rows"], 1);
            assert_eq!(latest["finite_committed"]["input_occurrences"], 6);
            assert_eq!(latest["finite_generation"]["number"], 2);
            let mut heartbeat = json!({"progress":latest});
            annotate_heartbeat(&mut heartbeat);
            assert_eq!(heartbeat["required_scope_progress"]["closed_rows"], 1);
            assert!(dashboard(&heartbeat)[1].contains("1/10 required rows closed"));
            assert!(dashboard(&heartbeat)[2].contains("local nodes 100/100"));
            assert!(dashboard(&heartbeat)[5].contains("not completed inputs"));
        }
        observe(&mut latest, checkpoint(3, 2));
        assert!(latest["finite_local"].is_null());
        assert_eq!(window_identity(&latest), None);
        assert_eq!(latest["finite_committed"]["input_occurrences"], 9);
        observe(
            &mut latest,
            json!({"operation":"finite_demand","event":"finite_backend_native",
            "row":2,"window":0,"snapshot":{"scheduled_nodes":7,"completed_nodes":1}}),
        );
        assert_eq!(local_snapshot(&latest).unwrap()["completed_nodes"], 1);
        assert_eq!(window_identity(&latest), Some((2, 0)));
        assert_eq!(latest["finite_committed"]["completed_work"], 300);
    }

    #[test]
    fn finite_restore_residency_and_failure_remain_visible_without_changing_legacy_events() {
        let mut latest = json!({});
        observe(
            &mut latest,
            json!({"operation":"finite_demand","event":"finite_backend_resident_hydrated",
            "status":"active","requested_bytes":16000000000u64,"resident":{"records":200}}),
        );
        let mut restored = checkpoint(5, 3);
        restored["event"] = json!("finite_checkpoint_restored");
        observe(&mut latest, restored);
        observe(
            &mut latest,
            json!({"operation":"finite_demand","event":"finite_backend_native",
            "row":3,"window":0,"snapshot":{"scheduled_nodes":9,"completed_nodes":9}}),
        );
        assert!(dashboard(&json!({"progress":latest}))[3].contains("resident Work active"));
        observe(
            &mut latest,
            json!({"operation":"finite_demand","event":"finite_backend_resident_fallback",
            "status":"declined","requested_bytes":16000000000u64}),
        );
        observe(
            &mut latest,
            json!({"operation":"finite_demand","event":"finished",
            "result":{"status":"cancelled","complete":false,"error":"cancelled"}}),
        );
        let lines = dashboard(&json!({"progress":latest}));
        assert!(lines[0].contains("cancelled"));
        assert!(lines[1].contains("3/10 required rows closed"));
        assert!(lines[3].contains("generation 5"));
        assert!(lines[3].contains("resident grant exhausted"));
        assert!(lines[5].contains("checkpoint retained"));
        let legacy = json!({"operation":"owner_domain_walk","completed_nodes":7});
        let mut unchanged = legacy.clone();
        retain(&latest, &mut unchanged);
        assert_eq!(unchanged, legacy);
        let mut heartbeat = json!({"progress":legacy});
        let before = heartbeat.clone();
        annotate_heartbeat(&mut heartbeat);
        assert_eq!(heartbeat, before);
    }
}
