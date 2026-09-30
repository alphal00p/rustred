//! Pure presentation of one bounded preparation snapshot.

use super::model::{Job, STALE_SECONDS, Status};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Table};

pub(super) fn clean(value: &str) -> String {
    value
        .chars()
        .filter(|c| !c.is_control())
        .take(1024)
        .collect()
}

fn count(value: Option<u64>) -> String {
    value.map_or_else(|| "?".into(), |n| n.to_string())
}
fn decimal(value: Option<f64>) -> String {
    value
        .filter(|v| v.is_finite() && *v >= 0.0)
        .map_or_else(|| "?".into(), |v| format!("{v:.1}"))
}
fn cpu(value: Option<f64>, budget: Option<u64>) -> String {
    format!("{}/{}", decimal(value), count(budget))
}
fn cpu_style(value: Option<f64>, budget: Option<u64>, color: bool) -> Style {
    let Some((value, budget)) = value
        .filter(|v| v.is_finite() && *v >= 0.0)
        .zip(budget.filter(|b| *b > 0))
        .filter(|_| color)
    else {
        return Style::default();
    };
    Style::default().fg(if value < budget as f64 * 0.5 {
        Color::Red
    } else if value < budget as f64 * 0.75 {
        Color::Yellow
    } else {
        Color::Green
    })
}
fn bytes(value: Option<u64>) -> String {
    value.map_or_else(
        || "?".into(),
        |n| format!("{:.1}GiB", n as f64 / (1024.0 * 1024.0 * 1024.0)),
    )
}
fn duration(value: Option<f64>) -> String {
    let Some(n) = value.filter(|v| v.is_finite() && *v >= 0.0) else {
        return "?".into();
    };
    let seconds = n as u64;
    format!(
        "{:02}:{:02}:{:02}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    )
}

fn title(status: &Status, now: f64) -> String {
    format!(
        "RustRed preparation | {} | {}{}",
        clean(&status.phase),
        clean(&status.state),
        if !status.terminal() && status.age(now) > STALE_SECONDS {
            " | STALE SUPERVISOR"
        } else {
            ""
        }
    )
}

fn summary(status: &Status) -> String {
    let aggregate = status.aggregate.as_ref();
    format!(
        "Parents {}/{} complete | {} active / {} queued | workers {} × {} = {} budget | elapsed {}",
        status.completed_jobs(),
        status.jobs.len(),
        count(aggregate.and_then(|a| a.active_jobs)),
        count(aggregate.and_then(|a| a.pending_jobs)),
        status.resources.jobs,
        status.resources.workers_per_job,
        status.resources.total_workers,
        duration(Some(status.elapsed_seconds))
    )
}

fn resources(status: &Status) -> String {
    let aggregate = status.aggregate.as_ref();
    let memory = &status.resources.memory;
    format!(
        "CPU {} cores | RAM {} / {} hard ({} soft; peak {}) | host available {} / floor {}",
        cpu(
            aggregate.and_then(|a| a.observed_cores),
            Some(status.resources.total_workers)
        ),
        bytes(aggregate.and_then(|a| a.sampled_rss_bytes)),
        bytes(memory.effective_hard_memory_bytes),
        bytes(memory.effective_soft_memory_bytes),
        bytes(aggregate.and_then(|a| a.sampled_peak_tree_rss_bytes)),
        bytes(aggregate.and_then(|a| a.host_available_bytes)),
        bytes(memory.host_memory_reserve_bytes)
    )
}

fn phase(job: &Job) -> String {
    let Some(native) = job.native() else {
        return format!(
            "{}; native {}",
            clean(&job.state),
            clean(
                job.native_progress
                    .as_ref()
                    .map_or("unknown", |n| if n.status == "fresh" {
                        "unknown"
                    } else {
                        n.status.as_str()
                    })
            )
        );
    };
    let latest = native.active_jobs.iter().max_by(|a, b| {
        a.last_event_elapsed_seconds
            .total_cmp(&b.last_event_elapsed_seconds)
    });
    latest.map_or_else(
        || {
            format!(
                "{} / {:?}{}",
                clean(&job.state),
                native.state,
                if native.details_truncated {
                    " (details capped)"
                } else {
                    ""
                }
            )
        },
        |active| {
            format!(
                "{} / {} [{}{}]",
                clean(&job.state),
                clean(&active.phase),
                native.active_jobs.len(),
                if native.details_truncated {
                    " tracked; capped"
                } else {
                    " active"
                }
            )
        },
    )
}

fn sectors(job: &Job) -> String {
    let done = job.native().and_then(|n| {
        (!n.counts.overflow)
            .then(|| n.counts.generated.checked_add(n.counts.reused))
            .flatten()
    });
    format!("{}/{}", count(done), job.metadata.selected_sectors.len())
}

fn checkpoint(job: &Job) -> String {
    job.native().map_or_else(
        || format!("? ({} files)", count(job.checkpoint_files_observed)),
        |n| {
            format!(
                "{} new / {} reused",
                n.counts.checkpointed_new, n.counts.reused
            )
        },
    )
}

fn frame_dimensions(job: &Job) -> String {
    let Some(native) = job.native() else {
        return "?".into();
    };
    let frame = native
        .active_jobs
        .iter()
        .filter_map(|job| job.frame.as_ref())
        .max_by_key(|frame| frame.source_rows);
    frame.map_or_else(
        || "—".into(),
        |frame| format!("{}r×{}c", frame.source_rows, frame.integral_columns),
    )
}

fn event_age(job: &Job, status_age: f64) -> Option<f64> {
    let native = job.native()?;
    let published_age = job.native_progress.as_ref()?.age_seconds?;
    Some(
        (native.elapsed_seconds - native.last_event_elapsed_seconds?).max(0.0)
            + published_age
            + status_age,
    )
}

pub(super) fn plain(status: &Status, now: f64, all_jobs: bool) -> String {
    let mut text = format!(
        "{} | {} | {} | status age {} | generation progress is not closure",
        title(status, now),
        summary(status),
        resources(status),
        duration(Some(status.age(now)))
    );
    for job in status.jobs.iter().filter(|job| all_jobs || job.active()) {
        let native = job.native();
        text.push_str(&format!("\n  {} | root {} | {} | sectors {} | checkpoint {} | new rules {} / residuals {} | max frame {} | CPU {} / RSS {} | event age {}",
            clean(&job.id), clean(&job.metadata.root), phase(job), sectors(job), checkpoint(job),
            count(native.map(|n| n.counts.rules_generated)), count(native.map(|n| n.counts.finite_residuals_generated)),
            frame_dimensions(job), cpu(job.observed_cores, job.workers), bytes(job.sampled_rss_bytes), duration(event_age(job, status.age(now)))));
    }
    if let Some(reason) = &status.stop_reason {
        text.push_str(&format!("\n  Stop: {}", clean(reason)));
    }
    text
}

pub(super) fn render(frame: &mut Frame<'_>, status: &Status, now: f64, color: bool) {
    let areas = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(2),
        Constraint::Min(2),
        Constraint::Length(2),
    ])
    .split(frame.area());
    let accent = if color {
        Style::default()
            .fg(if status.failed() {
                Color::Red
            } else if !status.terminal() && status.age(now) > STALE_SECONDS {
                Color::Yellow
            } else {
                Color::Cyan
            })
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    frame.render_widget(Paragraph::new(title(status, now)).style(accent), areas[0]);
    frame.render_widget(Paragraph::new(summary(status)), areas[1]);
    let aggregate = status.aggregate.as_ref();
    let memory = &status.resources.memory;
    let cpu_line = Span::styled(
        format!(
            "CPU {} cores",
            cpu(
                aggregate.and_then(|a| a.observed_cores),
                Some(status.resources.total_workers)
            )
        ),
        cpu_style(
            aggregate.and_then(|a| a.observed_cores),
            Some(status.resources.total_workers),
            color,
        ),
    );
    let resource_lines = vec![
        Line::from(vec![
            cpu_line,
            Span::raw(format!(
                " | RAM {} / {} hard ({} soft)",
                bytes(aggregate.and_then(|a| a.sampled_rss_bytes)),
                bytes(memory.effective_hard_memory_bytes),
                bytes(memory.effective_soft_memory_bytes)
            )),
        ]),
        Line::from(format!(
            "RAM peak {} | host available {} / floor {}",
            bytes(aggregate.and_then(|a| a.sampled_peak_tree_rss_bytes)),
            bytes(aggregate.and_then(|a| a.host_available_bytes)),
            bytes(memory.host_memory_reserve_bytes)
        )),
    ];
    frame.render_widget(Paragraph::new(resource_lines), areas[2]);
    let wide = areas[3].width >= 125;
    let narrow = areas[3].width < 70;
    let capacity = areas[3].height.saturating_sub(1) as usize;
    let ordered = status
        .jobs
        .iter()
        .filter(|job| job.active())
        .chain(
            status
                .jobs
                .iter()
                .filter(|job| !job.active() && job.state == "pending"),
        )
        .chain(
            status
                .jobs
                .iter()
                .filter(|job| !job.active() && job.state != "pending"),
        );
    let mut rows = Vec::with_capacity(capacity.min(status.jobs.len()));
    for job in ordered.take(capacity) {
        let native = job.native();
        let mut cells = vec![
            Cell::from(clean(&job.id)),
            Cell::from(phase(job)),
            Cell::from(sectors(job)),
        ];
        if !narrow {
            cells.push(Cell::from(native.map_or_else(
                || "? / ?".into(),
                |n| format!("{} / {}", n.counts.checkpointed_new, n.counts.reused),
            )));
        }
        if wide {
            cells.extend([
                Cell::from(count(native.map(|n| n.counts.rules_generated))),
                Cell::from(count(native.map(|n| n.counts.finite_residuals_generated))),
                Cell::from(frame_dimensions(job)),
            ]);
        }
        cells.push(
            Cell::from(cpu(job.observed_cores, job.workers)).style(cpu_style(
                job.observed_cores,
                job.workers,
                color,
            )),
        );
        if !narrow {
            cells.push(Cell::from(bytes(job.sampled_rss_bytes)));
        }
        if wide {
            cells.push(Cell::from(duration(event_age(job, status.age(now)))));
        }
        let style = if color && job.state == "failed" {
            Style::default().fg(Color::Red)
        } else if color && matches!(job.state.as_str(), "completed" | "retained") {
            Style::default().fg(Color::Green)
        } else {
            Style::default()
        };
        rows.push(Row::new(cells).style(style));
    }
    let mut headings = vec!["Parent", "Phase", "Sectors"];
    let mut widths = vec![
        Constraint::Length(12),
        Constraint::Min(if wide { 18 } else { 12 }),
        Constraint::Length(7),
    ];
    if !narrow {
        headings.push("CP new/reuse");
        widths.push(Constraint::Length(13));
    }
    if wide {
        headings.extend(["Rules+", "Resid+", "Max frame"]);
        widths.extend([
            Constraint::Length(6),
            Constraint::Length(6),
            Constraint::Length(11),
        ]);
    }
    headings.push("CPU/budget");
    widths.push(Constraint::Length(10));
    if !narrow {
        headings.push("RSS");
        widths.push(Constraint::Length(8));
    }
    if wide {
        headings.push("Event age");
        widths.push(Constraint::Length(9));
    }
    frame.render_widget(
        Table::new(rows, widths)
            .header(Row::new(headings).style(accent))
            .column_spacing(1),
        areas[3],
    );
    let hidden = status.jobs.len().saturating_sub(capacity);
    let footer = status.stop_reason.as_deref().map(clean).unwrap_or_else(|| {
        format!(
            "{} hidden parents | status age {} | CP = sector checkpoint, not in-sector resume",
            hidden,
            duration(Some(status.age(now)))
        )
    });
    frame.render_widget(Paragraph::new(format!("{footer}\nProgress ≠ closure; Rules+/Resid+ are new this invocation. Read-only; campaign signals still stop work.")), areas[4]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};
    #[test]
    fn presentation_removes_terminal_controls_without_losing_unicode() {
        assert_eq!(clean("a\x1b[31m\nβ\t\r"), "a[31mβ");
        assert_eq!(duration(None), "?");
        assert_eq!(decimal(Some(f64::NAN)), "?");
    }

    #[test]
    fn table_handles_tiny_narrow_and_wide_viewports_without_color() {
        let value = super::super::model::tests::fixture();
        let status = Status::parse(&serde_json::to_vec(&value).unwrap()).unwrap();
        for (width, height) in [(1, 1), (40, 10), (90, 16), (160, 24)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal
                .draw(|frame| render(frame, &status, 101.0, false))
                .unwrap();
            assert_eq!(terminal.backend().buffer().area.width, width);
        }
        let line = plain(&status, 101.0, true);
        assert!(line.contains("sectors ?/1"));
        assert!(line.contains("rules ? / residuals ?"));
        assert!(!line.contains('\x1b'));
    }

    #[test]
    fn retained_parent_is_visible_without_fabricated_native_totals() {
        let mut value = super::super::model::tests::fixture();
        value["jobs"][0]["state"] = "retained".into();
        let status = Status::parse(&serde_json::to_vec(&value).unwrap()).unwrap();
        let line = plain(&status, 101.0, true);
        assert!(line.contains("Parents 1/1 complete"));
        assert!(line.contains("retained; native unknown"));
        assert!(line.contains("rules ? / residuals ?"));
    }

    #[test]
    fn ninety_column_table_keeps_observed_budget_and_ram_visible() {
        let mut value = super::super::model::tests::fixture();
        value["jobs"][0]["state"] = "running".into();
        value["jobs"][0]["workers"] = 2.into();
        value["jobs"][0]["observed_cores"] = 1.5.into();
        value["jobs"][0]["sampled_rss_bytes"] = (3_u64 * 1024 * 1024 * 1024).into();
        let status = Status::parse(&serde_json::to_vec(&value).unwrap()).unwrap();
        let mut terminal = Terminal::new(TestBackend::new(90, 16)).unwrap();
        terminal
            .draw(|frame| render(frame, &status, 101.0, true))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let text = buffer
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        for expected in [
            "Parent",
            "Sectors",
            "CP new/reuse",
            "CPU/budget",
            "RSS",
            "1.5/2",
            "3.0GiB",
        ] {
            assert!(text.contains(expected), "missing {expected:?} in {text}");
        }
        assert!(!text.contains("Ctrl-C exits viewer only"));
        // Resource sampling color is independently attached to CPU cells.
        assert!(
            buffer
                .content
                .iter()
                .any(|cell| cell.symbol() == "1" && cell.fg == Color::Green)
        );
    }

    #[test]
    fn cpu_thresholds_use_observed_cores_over_reserved_budget_and_unknown_is_neutral() {
        assert_eq!(cpu_style(Some(1.99), Some(4), true).fg, Some(Color::Red));
        assert_eq!(cpu_style(Some(2.0), Some(4), true).fg, Some(Color::Yellow));
        assert_eq!(cpu_style(Some(3.0), Some(4), true).fg, Some(Color::Green));
        assert_eq!(cpu_style(None, Some(4), true), Style::default());
        assert_eq!(cpu_style(Some(f64::NAN), Some(4), true), Style::default());
        assert_eq!(cpu_style(Some(4.0), Some(0), true), Style::default());
        assert_eq!(cpu_style(Some(4.0), Some(4), false), Style::default());
    }

    #[test]
    fn footer_does_not_claim_terminal_control_c_only_stops_the_attached_viewer() {
        let value = super::super::model::tests::fixture();
        let status = Status::parse(&serde_json::to_vec(&value).unwrap()).unwrap();
        let mut terminal = Terminal::new(TestBackend::new(160, 16)).unwrap();
        terminal
            .draw(|frame| render(frame, &status, 101.0, false))
            .unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(text.contains("Read-only; campaign signals still stop work."));
        assert!(!text.contains("Ctrl-C exits viewer only"));
    }
}
