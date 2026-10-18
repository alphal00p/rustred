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
mod chase;
pub(super) use chase::ChaseProgress;
mod eta;
pub(super) use eta::InputEta;

pub(in crate::cli::progress) const HEIGHT: u16 = 24;

fn memory_gb(value: &Value) -> String {
    value.as_u64().map_or_else(
        || "—".into(),
        |bytes| format!("{:.2} GB", bytes as f64 / 1e9),
    )
}

fn number(value: &Value) -> String {
    value
        .as_u64()
        .map(|n| n.to_string())
        .or_else(|| {
            value
                .as_str()
                .filter(|digits| !digits.is_empty() && digits.bytes().all(|c| c.is_ascii_digit()))
                .map(str::to_owned)
        })
        .map_or_else(
            || "—".into(),
            |digits| {
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

fn input_occurrences(event: &Value) -> String {
    format!(
        "{} / {}",
        number(&event["finite_committed"]["input_occurrences"]),
        number(&event["finite_totals"]["input_occurrences"])
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
        "finite_backend_native"
            if event["finite_group"]["windows_count"].as_u64().unwrap_or(1) > 1 =>
        {
            "Solving current window group".into()
        }
        "finite_backend_native" => "Solving current window".into(),
        "finite_backend_group_planning" => "Planning shared window group".into(),
        "finite_backend_group_materializing" => "Materializing exact starting inputs".into(),
        "finite_backend_group_planned" => "Binding immutable checkpoint".into(),
        "finite_backend_trace_started" => "Admitting shared starting inputs".into(),
        "finite_backend_before_commit" => "Preparing atomic commit".into(),
        "finite_backend_commit_started" => "Writing atomic checkpoint".into(),
        "finite_backend_group_cleanup" => "Checkpoint safe · releasing prior batch metadata".into(),
        "finite_worker_pool_ready" => "Persistent worker pool ready".into(),
        "finite_checkpoint_saved" => "Checkpoint committed".into(),
        "finite_checkpoint_restored" => "Checkpoint restored".into(),
        "finite_cleanup_started" => "Solve complete · releasing working memory".into(),
        "finite_cleanup_finished" => "Working memory released · handing off".into(),
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

fn convergence_color(chase: &Value, field: &str) -> Color {
    if chase["snapshot_stale"] == true {
        return Color::Gray;
    }
    match chase[field]["value"].as_f64() {
        Some(value) if value > 0.5 => Color::Red,
        Some(value) if value > 0. => Color::Yellow,
        Some(value) if value < 0. => Color::Green,
        _ => Color::Gray,
    }
}

fn convergence_text(chase: &Value, field: &str) -> String {
    let metric = &chase[field];
    let label = match metric["status"].as_str() {
        Some("available") => metric["value"]
            .as_f64()
            .map_or_else(|| "—".into(), |value| format!("{value:+.3}")),
        Some("warming_up") => "warming up".into(),
        Some("history_gap") => "history gap".into(),
        _ => "—".into(),
    };
    if chase["snapshot_stale"] == true {
        format!("{label} · stale")
    } else {
        label
    }
}

fn chase_elapsed(chase: &Value) -> String {
    chase["elapsed_seconds"].as_f64().map_or_else(
        || "—".into(),
        |elapsed| {
            let label = duration(elapsed);
            if chase["snapshot_stale"] == true {
                format!("{label} · stale")
            } else {
                label
            }
        },
    )
}

fn input_eta_text(eta: &Value) -> String {
    match eta["status"].as_str() {
        Some("complete") => "00:00:00".into(),
        Some("available") => eta["remaining_seconds"]
            .as_f64()
            .filter(|seconds| seconds.is_finite() && *seconds >= 0.)
            .map_or_else(
                || "—".into(),
                |seconds| {
                    if seconds >= u64::MAX as f64 {
                        format!("~{seconds:.2e}s")
                    } else {
                        format!("~{}", duration(seconds))
                    }
                },
            ),
        Some("warming_up") => "warming up".into(),
        _ => "—".into(),
    }
}

fn input_average_text(eta: &Value) -> String {
    eta["inputs_per_second"]
        .as_f64()
        .filter(|rate| rate.is_finite() && *rate >= 0.)
        .map_or_else(|| "—".into(), |rate| format!("{rate:.3}"))
}

fn timing_pair(timings: &Value, left: &str, right: &str) -> String {
    let one = |field: &str| {
        timings[field]
            .as_f64()
            .filter(|n| n.is_finite() && *n >= 0.)
            .map_or_else(|| "—".into(), |n| format!("{n:.3}s"))
    };
    format!("{} / {}", one(left), one(right))
}

pub(super) fn plain_status(record: &Value) -> String {
    let event = &record["progress"];
    let committed = &event["finite_committed"];
    let chase = &record["finite_chase"];
    let eta = &record["finite_input_eta"];
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
    let status = format!(
        "RustRed finite | {} | committed {}/{} rows | inputs {} occurrences (overlap counted) | rough input ETA {} at {}/s (whole invocation; publication excluded) | generation {} / {} committed windows | current group {} windows / {} inputs | last commit new Work {} / reuse hits {} | chase gap {} / elapsed {} | convergence 5 min {} / 30 min {} (negative: shrinking gap, not closure) | CPU {}/{} cores | RSS {} | process RAM cap {} | stop threshold {} | host available {} / {} total | elapsed {}{error}",
        stage(event),
        number(&committed["exhausted_required_rows"]),
        number(&committed["required_rows"]),
        input_occurrences(event),
        input_eta_text(eta),
        input_average_text(eta),
        number(&event["finite_generation"]["number"]),
        number(&committed["committed_windows"]),
        number(&event["finite_group"]["windows_count"]),
        number(&event["finite_group"]["inputs"]),
        number(&event["finite_last_window"]["new_work"]),
        number(&event["finite_last_window"]["baseline_hits"]),
        number(&chase["gap"]),
        chase_elapsed(chase),
        convergence_text(chase, "five_minutes"),
        convergence_text(chase, "thirty_minutes"),
        cpu,
        number(&record["reserved_workers"]),
        memory_gb(&record["process_rss_bytes"]),
        memory_gb(&record["process_memory_limit_bytes"]),
        memory_gb(&record["process_memory_soft_bytes"]),
        memory_gb(&record["host_available_bytes"]),
        memory_gb(&record["host_total_bytes"]),
        duration(record["elapsed_seconds"].as_f64().unwrap_or(0.))
    );
    format!(
        "{status} | last trace/commit {} | last plan/total {}",
        timing_pair(
            &event["finite_last_window"]["timings"],
            "trace_seconds",
            "commit_seconds"
        ),
        timing_pair(
            &event["finite_last_window"]["timings"],
            "planning_seconds",
            "total_seconds"
        )
    )
}

/// One bounded presentation of native counters. It never derives closure from
/// transient node counts, and ratatui clips every cell to the current viewport.
pub(in crate::cli::progress) fn render(frame: &mut Frame<'_>, record: &Value, color: bool) {
    let event = &record["progress"];
    let committed = &event["finite_committed"];
    let local = &event["finite_local"];
    let snapshot = &local["snapshot"];
    let chase = &record["finite_chase"];
    let eta = &record["finite_input_eta"];
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
        Constraint::Length(16),
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
    let memory = memory_gb(&record["process_rss_bytes"]);
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
            input_occurrences(event),
        )
    };
    let metrics = [
        first,
        (
            "ETA · run average",
            input_eta_text(eta),
            "Inputs/s · run avg",
            input_average_text(eta),
        ),
        (
            "Gen / closed win",
            format!(
                "{} / {}",
                number(&event["finite_generation"]["number"]),
                number(&committed["committed_windows"])
            ),
            "Auxiliary closed",
            format!(
                "{} / {}",
                number(&committed["exhausted_auxiliary_rows"]),
                number(&committed["auxiliary_rows"])
            ),
        ),
        (
            "Last new Work",
            number(&event["finite_last_window"]["new_work"]),
            "Last reuse hits",
            number(&event["finite_last_window"]["baseline_hits"]),
        ),
        (
            "Last trace/commit",
            timing_pair(
                &event["finite_last_window"]["timings"],
                "trace_seconds",
                "commit_seconds",
            ),
            "Last plan/total",
            timing_pair(
                &event["finite_last_window"]["timings"],
                "planning_seconds",
                "total_seconds",
            ),
        ),
        (
            "Row (0-based)",
            number(position("row")),
            "Window (0-based)",
            number(position("window")),
        ),
        (
            "Windows / group",
            number(&event["finite_group"]["windows_count"]),
            "Group inputs",
            number(&event["finite_group"]["inputs"]),
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
            "Chase gap",
            number(&chase["gap"]),
            "Chase elapsed",
            chase_elapsed(chase),
        ),
        (
            "Convergence · 5 min",
            convergence_text(chase, "five_minutes"),
            "Convergence · 30 min",
            convergence_text(chase, "thirty_minutes"),
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
            "Process RAM cap",
            memory_gb(&record["process_memory_limit_bytes"]),
            "Stop threshold",
            memory_gb(&record["process_memory_soft_bytes"]),
        ),
        (
            "Host available",
            memory_gb(&record["host_available_bytes"]),
            "Host total",
            memory_gb(&record["host_total_bytes"]),
        ),
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
        let (left_tone, right_tone) = match left {
            "CPU / reserved" => (cpu_tone, cpu_tone),
            "Convergence · 5 min" => (
                convergence_color(chase, "five_minutes"),
                convergence_color(chase, "thirty_minutes"),
            ),
            _ => (Color::White, Color::White),
        };
        if wide {
            Row::new(vec![
                Cell::from(left).style(paint(Color::Gray)),
                value(lv, left_tone),
                Cell::from(right).style(paint(Color::Gray)),
                value(rv, right_tone),
            ])
        } else {
            let label = match left {
                "Saved Work" => "Work / inputs",
                "Stage completed" => "Stage done/total",
                "ETA · run average" => "ETA / inputs/s",
                "Gen / closed win" => "Gen,win / aux",
                "Last new Work" => "Last new / reuse",
                "Row (0-based)" => "Row / window (0)",
                "Windows / group" => "Group win/inputs",
                "Expanded nodes" => "Expanded / seen",
                "Queued nodes" => "Queue / busy",
                "Chase gap" => "Gap / chase time",
                "Convergence · 5 min" => "Converg 5m / 30m",
                "CPU / reserved" => "CPU / reserved",
                "Elapsed" => "Elapsed / RSS",
                "Process RAM cap" => "RAM cap / stop",
                "Host available" => "Host avail/total",
                _ => "Rate / age",
            };
            Row::new(vec![
                Cell::from(label).style(paint(Color::Gray)),
                Cell::from(
                    Text::from(Line::from(vec![
                        Span::styled(lv, paint(left_tone)),
                        Span::raw("  │  "),
                        Span::styled(rv, paint(right_tone)),
                    ]))
                    .alignment(Alignment::Right),
                ),
            ])
        }
    });
    let widths = if wide {
        vec![
            Constraint::Length(20),
            Constraint::Fill(1),
            Constraint::Length(20),
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
        Some("releasing") => "Work cache: releasing memory · checkpoint safe",
        Some("released") => "Work cache: released",
        _ => "Work cache: preparing",
    };
    frame.render_widget(
        Paragraph::new(residency).style(paint(Color::Blue)),
        sections[4],
    );
    let footer = failure.map_or_else(
        || if matches!(event["event"].as_str(), Some("finite_cleanup_started" | "finite_cleanup_finished")) {
            "Solve checkpoint is safe. Releasing working memory does not change coverage; artifact publication follows separately.".to_string()
        } else {
            "Chase covers the current group; rows advance only after its atomic commit. ETA is rough and excludes publication. Convergence < 0 means shrinking gap, not closure.".to_string()
        },
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
            "finite_totals",
            "finite_last_window",
            "finite_residency",
            "finite_local",
            "finite_group",
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
            if let Some(total) = event.get("input_occurrences_total").cloned() {
                event["finite_totals"] = json!({"input_occurrences":total});
            }
            if event["window"].is_object() {
                event["finite_last_window"] = event["window"].clone();
                if event["window"]["resident"].is_object() {
                    event["finite_residency"]["resident"] = event["window"]["resident"].clone();
                }
            }
            // The next cursor may designate a new window. Never display the
            // preceding window's local completion as that window's progress.
            event["finite_local"] = Value::Null;
            event["finite_group"] = Value::Null;
        }
        Some("finite_backend_group_planned") => {
            event["finite_group"] = event["group"].clone();
            event["finite_local"] = Value::Null;
        }
        Some("finite_backend_group_planning") => {
            event["finite_group"] = Value::Null;
            event["finite_local"] = Value::Null;
        }
        Some("finite_backend_native") => {
            if event["group"].is_object() {
                event["finite_group"] = event["group"].clone();
            }
            event["finite_local"] = json!({"row":event["row"],"window":event["window"],
                "snapshot":event.get("snapshot").unwrap_or(&event["native"])});
        }
        Some("finite_backend_resident_hydrated" | "finite_backend_resident_fallback") => {
            event["finite_residency"] = json!({"status":event["status"],
                "requested_bytes":event["requested_bytes"],"resident":event["resident"],
                "fallback":event["event"] == "finite_backend_resident_fallback"});
        }
        Some("finite_cleanup_started" | "finite_cleanup_finished") => {
            event["finite_local"] = Value::Null;
            event["finite_group"] = Value::Null;
            event["finite_residency"] = json!({"status":if event["event"] == "finite_cleanup_started" {
                "releasing"
            } else {
                "released"
            }});
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
            if !event["finite_totals"].is_object()
                && let Some(total) = event["result"].get("input_occurrences_total").cloned()
            {
                event["finite_totals"] = json!({"input_occurrences":total});
            }
            event["finite_local"] = Value::Null;
            event["finite_group"] = Value::Null;
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
    record["input_occurrences_progress"] = json!({
        "processed":record["progress"]["finite_committed"]["input_occurrences"],
        "total":record["progress"]["finite_totals"]["input_occurrences"],
        "counts_occurrences_including_overlap":true,
        "from_native_committed_checkpoint":true});
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
        Some("releasing") => "releasing Work memory (checkpoint safe)",
        Some("released") => "Work memory released",
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
    fn finite_group_metadata_survives_chase_and_resets_after_atomic_commit() {
        let mut latest = json!({});
        observe(&mut latest, checkpoint(1, 0));
        let group = json!({"windows_count":64,"inputs":320,"distinct_inputs":300,"last_row":2,"last_window":31});
        observe(
            &mut latest,
            json!({"operation":"finite_demand","event":"finite_backend_group_planned",
            "row":0,"window":0,"group":group}),
        );
        assert_eq!(latest["finite_group"]["windows_count"], 64);
        assert!(latest["finite_local"].is_null());
        observe(
            &mut latest,
            json!({"operation":"finite_demand","event":"finite_backend_native",
            "row":0,"window":0,"group":group,"snapshot":{"scheduled_nodes":100,"completed_nodes":50}}),
        );
        assert_eq!(stage(&latest), "Solving current window group");
        assert_eq!(window_identity(&latest), Some((0, 0)));
        observe(
            &mut latest,
            json!({"operation":"finite_demand","event":"finite_backend_before_commit",
            "row":0,"window":0,"group":group}),
        );
        assert_eq!(latest["finite_group"]["inputs"], 320);
        assert_eq!(stage(&latest), "Preparing atomic commit");
        let plain = plain_status(&json!({"progress":latest}));
        assert!(
            plain.contains("current group 64 windows / 320 inputs"),
            "{plain}"
        );
        let mut saved = checkpoint(2, 3);
        saved["window"] = json!({"row":0,"windows_count":64,"last_row":2,"last_window":31,
            "inputs":320,"new_work":80,"baseline_hits":300});
        observe(&mut latest, saved);
        assert!(latest["finite_group"].is_null());
        assert!(latest["finite_local"].is_null());
        assert_eq!(latest["finite_last_window"]["windows_count"], 64);
    }

    #[test]
    fn finite_table_resizes_and_keeps_committed_progress_distinct_from_window_counts() {
        let mut latest = json!({});
        let mut saved = checkpoint(4_129, 3);
        saved["window"] = json!({"row":3,"inputs":100,"new_work":777,"baseline_hits":333});
        observe(&mut latest, saved);
        observe(
            &mut latest,
            json!({"operation":"finite_demand", "event":"finite_backend_native",
            "row":3,"window":17,"snapshot":{"completed_nodes":12000,"scheduled_nodes":24000,
            "queued_nodes":8000,"active_nodes":4,"workers":8,"rule_applications":999,"transport_calls":11},
            "group":{"windows_count":64,"inputs":1024,"distinct_inputs":1000,"last_row":4,"last_window":7}}),
        );
        let record = json!({"progress":latest,"elapsed_seconds":3723.0,"process_rss_bytes":8u64 << 30,
            "process_cpu_cores":6.5,"reserved_workers":8,
            "process_memory_limit_bytes":600_000_000_000u64,"process_memory_soft_bytes":570_000_000_000u64,
            "host_available_bytes":100_000_000_000u64,"host_total_bytes":1_024_000_000_000u64,
            "recent_nodes_per_second":125.5,"progress_age_seconds":0.2,
            "finite_input_eta":{"status":"available","remaining_seconds":7200.,"inputs_per_second":10.},
            "finite_chase":{"status":"active","elapsed_seconds":1832.,"gap":12000,
                "five_minutes":{"status":"available","value":-0.25},
                "thirty_minutes":{"status":"available","value":0.75}}});
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
                for label in [
                    "Process RAM cap",
                    "Stop threshold",
                    "Host available",
                    "Host total",
                    "Input occurrences",
                    "ETA · run average",
                    "Inputs/s · run avg",
                    "Gen / closed win",
                    "Windows / group",
                    "Group inputs",
                    "Last new Work",
                    "Last reuse hits",
                    "Chase gap",
                    "Chase elapsed",
                    "Convergence · 5 min",
                    "Convergence · 30 min",
                ] {
                    assert!(text.contains(label), "{text}");
                }
            }
            if width >= 80 {
                assert!(text.contains("3 / 10 required rows closed"), "{text}");
                assert!(text.contains("12,000"), "{text}");
                assert!(text.contains("24,000"), "{text}");
                assert!(text.contains("4,129"), "{text}");
                assert!(text.contains("4,128"), "{text}");
                assert!(text.contains("12,387 / 50,000"), "{text}");
                assert!(text.contains("777") && text.contains("333"), "{text}");
                assert!(text.contains("1,024"), "{text}");
                assert!(text.contains("01:02:03"), "{text}");
                assert!(
                    text.contains("~02:00:00") && text.contains("10.000"),
                    "{text}"
                );
                assert!(text.contains("00:30:32"), "{text}");
                assert!(text.contains("-0.250") && text.contains("+0.750"), "{text}");
                assert!(text.contains("6.50 / 8"), "{text}");
                assert!(text.contains("81.2%"), "{text}");
                for memory in [
                    "8.59 GB",
                    "600.00 GB",
                    "570.00 GB",
                    "100.00 GB",
                    "1024.00 GB",
                ] {
                    assert!(text.contains(memory), "{text}");
                }
                assert!(
                    text.contains("125.5") && text.contains("0.2s ago"),
                    "{text}"
                );
                assert!(text.contains("╭") && text.contains("╰"), "{text}");
            }
        }
    }

    #[test]
    fn finite_input_eta_rendering_is_explicitly_rough_and_shared_with_plain_output() {
        let eta = json!({"status":"available","remaining_seconds":90.,"inputs_per_second":0.25});
        assert_eq!(input_eta_text(&eta), "~00:01:30");
        assert_eq!(input_average_text(&eta), "0.250");
        assert_eq!(input_eta_text(&json!({"status":"complete"})), "00:00:00");
        assert_eq!(
            input_eta_text(&json!({"status":"warming_up"})),
            "warming up"
        );
        assert_eq!(input_eta_text(&Value::Null), "—");
        let huge = input_eta_text(&json!({"status":"available","remaining_seconds":1e100}));
        assert!(huge.contains('e') && huge.ends_with('s'), "{huge}");
        assert_eq!(
            input_eta_text(&json!({"status":"available","remaining_seconds":-1.})),
            "—"
        );
        let text = plain_status(&json!({"progress":{},"finite_input_eta":eta}));
        assert!(
            text.contains("rough input ETA ~00:01:30 at 0.250/s"),
            "{text}"
        );
        assert!(
            text.contains("whole invocation; publication excluded"),
            "{text}"
        );
    }

    #[test]
    fn finite_convergence_color_thresholds_and_staleness_are_explicit() {
        for (value, expected) in [
            (-1., Color::Green),
            (-0.001, Color::Green),
            (0., Color::Gray),
            (0.001, Color::Yellow),
            (0.5, Color::Yellow),
            (0.501, Color::Red),
            (1., Color::Red),
        ] {
            let chase = json!({"five_minutes":{"status":"available","value":value}});
            assert_eq!(convergence_color(&chase, "five_minutes"), expected);
        }
        let mut chase = json!({"elapsed_seconds":301.,
            "five_minutes":{"status":"available","value":-0.25},
            "thirty_minutes":{"status":"warming_up"}});
        assert_eq!(convergence_text(&chase, "five_minutes"), "-0.250");
        assert_eq!(convergence_text(&chase, "thirty_minutes"), "warming up");
        chase["snapshot_stale"] = json!(true);
        assert_eq!(convergence_text(&chase, "five_minutes"), "-0.250 · stale");
        assert_eq!(convergence_color(&chase, "five_minutes"), Color::Gray);
        assert_eq!(chase_elapsed(&chase), "00:05:01 · stale");
        assert_eq!(chase_elapsed(&json!({"status":"inactive"})), "—");
        let plain = plain_status(&json!({"progress":{"operation":"finite_demand"},
            "finite_chase":chase}));
        assert!(plain.contains("convergence 5 min -0.250 · stale / 30 min warming up · stale"));
        assert!(plain.contains("not closure"));
    }

    #[test]
    fn finite_plain_status_reports_host_memory_separately_from_the_requested_process_cap() {
        let record = json!({"progress":{"operation":"finite_demand"},
            "process_rss_bytes":8_000_000_000u64,
            "process_memory_limit_bytes":600_000_000_000u64,
            "process_memory_soft_bytes":570_000_000_000u64,
            "host_available_bytes":100_000_000_000u64,
            "host_total_bytes":1_024_000_000_000u64});
        let plain = plain_status(&record);
        assert!(
            plain.contains("RSS 8.00 GB | process RAM cap 600.00 GB | stop threshold 570.00 GB"),
            "{plain}"
        );
        assert!(
            plain.contains("host available 100.00 GB / 1024.00 GB total"),
            "{plain}"
        );
        let unknown = plain_status(&json!({"progress":{"operation":"finite_demand"}}));
        assert!(
            unknown.contains("process RAM cap — | stop threshold — | host available — / — total"),
            "{unknown}"
        );
        assert_eq!(memory_gb(&json!(0)), "0.00 GB");
        assert_eq!(memory_gb(&Value::Null), "—");
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
            "input_occurrences_total":"50000",
            "committed":{"required_rows":10,"exhausted_required_rows":done,
                "required_complete":false,"auxiliary_rows":2,"exhausted_auxiliary_rows":1,
                "current_row":done,"current_window":0,"completed_work":generation*100,
                "input_occurrences":generation*3,"committed_windows":generation.saturating_sub(1)}})
    }
    fn observe(previous: &mut Value, mut event: Value) {
        retain(previous, &mut event);
        *previous = event;
    }

    #[test]
    fn cleanup_is_distinct_from_solving_and_publication_with_checkpoint_retained() {
        let mut latest = json!({});
        let mut saved = checkpoint(9, 10);
        saved["committed"]["required_complete"] = json!(true);
        observe(&mut latest, saved);
        for (event, text, residency) in [
            (
                "finite_cleanup_started",
                "releasing working memory",
                "releasing",
            ),
            (
                "finite_cleanup_finished",
                "Working memory released",
                "released",
            ),
        ] {
            observe(
                &mut latest,
                json!({"operation":"finite_demand","event":event}),
            );
            assert_eq!(latest["finite_committed"]["exhausted_required_rows"], 10);
            assert_eq!(latest["finite_generation"]["number"], 9);
            assert!(latest["finite_local"].is_null());
            assert!(latest["finite_group"].is_null());
            assert_eq!(latest["finite_residency"]["status"], residency);
            assert!(stage(&latest).contains(text));
            assert!(plain_status(&json!({"progress":latest})).contains(text));
            assert_eq!(checkpoint_note(&latest), "checkpoint retained");
        }
    }

    #[test]
    fn finite_totals_and_last_committed_window_survive_native_activity() {
        let mut latest = json!({});
        let mut initial = checkpoint(1, 0);
        initial["input_occurrences_total"] = json!("7");
        initial["committed"]["input_occurrences"] = json!(0);
        observe(&mut latest, initial);
        let mut saved = checkpoint(2, 0);
        saved
            .as_object_mut()
            .unwrap()
            .remove("input_occurrences_total");
        saved["committed"]["input_occurrences"] = json!(3);
        saved["window"] = json!({"row":0,"inputs":3,"new_work":2,"baseline_hits":1});
        observe(&mut latest, saved);
        for event in [
            json!({"operation":"finite_demand","event":"finite_backend_native",
                "row":1,"window":0,"snapshot":{"completed_nodes":500,"scheduled_nodes":500}}),
            json!({"operation":"finite_demand","event":"finite_backend_resident_hydrated",
                "status":"active"}),
            json!({"operation":"finite_demand","event":"finished","result":{"status":"cancelled"}}),
        ] {
            observe(&mut latest, event);
            assert_eq!(latest["finite_totals"]["input_occurrences"], "7");
            assert_eq!(latest["finite_last_window"]["new_work"], 2);
            assert_eq!(latest["finite_last_window"]["baseline_hits"], 1);
            assert_eq!(input_occurrences(&latest), "3 / 7");
        }
        let mut heartbeat = json!({"progress":latest});
        annotate_heartbeat(&mut heartbeat);
        assert_eq!(heartbeat["input_occurrences_progress"]["processed"], 3);
        assert_eq!(heartbeat["input_occurrences_progress"]["total"], "7");
        assert_eq!(
            heartbeat["input_occurrences_progress"]["counts_occurrences_including_overlap"],
            true
        );
        let plain = plain_status(&heartbeat);
        assert!(
            plain.contains("inputs 3 / 7 occurrences (overlap counted)"),
            "{plain}"
        );
        assert!(
            plain.contains("generation 2 / 1 committed windows"),
            "{plain}"
        );
        assert!(
            plain.contains("last commit new Work 2 / reuse hits 1"),
            "{plain}"
        );
    }

    #[test]
    fn finite_total_decimal_strings_preserve_arbitrary_precision() {
        assert_eq!(
            number(&json!("18446744073709551616000")),
            "18,446,744,073,709,551,616,000"
        );
        for invalid in ["", "-1", "1e6", "12\n", "\u{1b}[2J"] {
            assert_eq!(number(&json!(invalid)), "—");
        }
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
