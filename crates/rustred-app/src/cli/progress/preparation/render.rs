//! Pure presentation of one bounded preparation snapshot.

use super::model::{Job, STALE_SECONDS, Status};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Cell, Gauge, Paragraph, Row, Table, Wrap};

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
    if status.resources.shared_pool {
        let pool = aggregate.and_then(|a| a.pool.as_ref());
        return format!(
            "Parents {}/{} complete | shared {}-worker pool | sectors {} active / {} queued | cap {} | elapsed {}",
            status.completed_jobs(),
            status.jobs.len(),
            status.resources.total_workers,
            count(pool.map(|p| p.running)),
            count(pool.map(|p| p.waiting_for_worker)),
            status
                .resources
                .max_active_sectors
                .map_or_else(|| "all".into(), |n| n.to_string()),
            duration(Some(status.elapsed_seconds))
        );
    }
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
    if matches!(job.state.as_str(), "interrupted" | "failed" | "not_started") {
        let last = job.native().and_then(|n| {
            n.active_jobs.iter().max_by(|a, b| {
                a.last_event_elapsed_seconds
                    .total_cmp(&b.last_event_elapsed_seconds)
            })
        });
        return last.map_or_else(
            || clean(&job.state),
            |last| format!("{} · last {}", clean(&job.state), clean(&last.phase)),
        );
    }
    if job.shared_process {
        if job.preparation_state.as_deref() == Some("preparing") {
            return "family preparation".into();
        }
        if job.state == "pending" {
            return "queued".into();
        }
    }
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
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(accent)
        .title(format!(
            " RustRed / {} / {} / {} ",
            clean(&status.phase).to_uppercase(),
            clean(&status.state).to_uppercase(),
            duration(Some(status.elapsed_seconds))
        ));
    let inner = block.inner(frame.area());
    frame.render_widget(block, frame.area());
    let areas = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(3),
        Constraint::Min(2),
        Constraint::Length(3),
    ])
    .split(inner);
    let ratio = if status.jobs.is_empty() {
        0.0
    } else {
        status.completed_jobs() as f64 / status.jobs.len() as f64
    };
    frame.render_widget(
        Gauge::default()
            .ratio(ratio)
            .gauge_style(accent)
            .label(format!(
                "PARENTS  {} / {} complete  ·  generation, not closure",
                status.completed_jobs(),
                status.jobs.len()
            )),
        areas[0],
    );
    let aggregate = status.aggregate.as_ref();
    let memory = &status.resources.memory;
    let used = aggregate.and_then(|a| a.sampled_rss_bytes);
    let mem_style = if color
        && used
            .zip(memory.effective_soft_memory_bytes)
            .is_some_and(|(u, s)| u >= s)
    {
        Style::default().fg(Color::Red)
    } else {
        Style::default()
    };
    let pool_detail = if status.resources.shared_pool {
        let pool = aggregate.and_then(|a| a.pool.as_ref());
        format!(
            "shared · {} sectors active / {} queued · cap {}",
            count(pool.map(|p| p.running)),
            count(pool.map(|p| p.waiting_for_worker)),
            status
                .resources
                .max_active_sectors
                .map_or_else(|| "all".into(), |n| n.to_string())
        )
    } else {
        format!(
            "{} slots × {} workers  ·  {} active / {} queued",
            status.resources.jobs,
            status.resources.workers_per_job,
            count(aggregate.and_then(|a| a.active_jobs)),
            count(aggregate.and_then(|a| a.pending_jobs))
        )
    };
    let stats = vec![
        Row::new(vec![
            Cell::from("CPU / reserved"),
            Cell::from(cpu(
                aggregate.and_then(|a| a.observed_cores),
                Some(status.resources.total_workers),
            ))
            .style(cpu_style(
                aggregate.and_then(|a| a.observed_cores),
                Some(status.resources.total_workers),
                color,
            )),
            Cell::from(pool_detail),
        ]),
        Row::new(vec![
            Cell::from("RAM / ceiling"),
            Cell::from(format!(
                "{} / {}",
                bytes(used),
                bytes(memory.effective_hard_memory_bytes)
            ))
            .style(mem_style),
            Cell::from(format!(
                "stop at {}  ·  peak {}",
                bytes(memory.effective_soft_memory_bytes),
                bytes(aggregate.and_then(|a| a.sampled_peak_tree_rss_bytes))
            )),
        ]),
        Row::new(vec![
            Cell::from("Host available"),
            Cell::from(bytes(aggregate.and_then(|a| a.host_available_bytes))),
            Cell::from(format!(
                "status age {}{}",
                duration(Some(status.age(now))),
                if !status.terminal() && status.age(now) > STALE_SECONDS {
                    " · STALE SUPERVISOR"
                } else {
                    ""
                }
            )),
        ]),
    ];
    frame.render_widget(
        Table::new(
            stats,
            [
                Constraint::Length(16),
                Constraint::Length(24),
                Constraint::Min(10),
            ],
        )
        .column_spacing(2),
        areas[1],
    );
    let wide = areas[2].width >= 125;
    let narrow = areas[2].width < 70;
    let capacity = areas[2].height.saturating_sub(2) as usize;
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
    for (index, job) in ordered.take(capacity).enumerate() {
        let native = job.native();
        let mut cells = vec![
            Cell::from(parent_label(job)),
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
        if job.shared_process {
            cells.push(Cell::from(if !job.active() {
                "0".into()
            } else if job.preparation_state.as_deref() == Some("preparing") {
                "prep".into()
            } else {
                native.map_or_else(|| "—".into(), |n| n.active_jobs.len().to_string())
            }));
        } else {
            cells.push(
                Cell::from(cpu(job.observed_cores, job.workers)).style(cpu_style(
                    job.observed_cores,
                    job.workers,
                    color,
                )),
            );
        }
        if !narrow {
            cells.push(Cell::from(if job.shared_process {
                "shared".into()
            } else {
                bytes(job.sampled_rss_bytes)
            }));
        }
        if wide {
            cells.push(Cell::from(duration(event_age(job, status.age(now)))));
        }
        let style = if color && job.state == "failed" {
            Style::default().fg(Color::Red)
        } else if color && matches!(job.state.as_str(), "completed" | "retained") {
            Style::default().fg(Color::Green)
        } else if color && index % 2 == 0 {
            Style::default().bg(Color::Rgb(25, 29, 39))
        } else {
            Style::default()
        };
        rows.push(Row::new(cells).style(style));
    }
    let mut headings = vec!["Parent", "Phase", "Sectors"];
    let mut widths = vec![
        Constraint::Length(16),
        Constraint::Min(if wide { 18 } else { 12 }),
        Constraint::Length(7),
    ];
    if !narrow {
        headings.push("CP new/reuse");
        widths.push(Constraint::Length(12));
    }
    if wide {
        headings.extend(["Rules+", "Resid+", "Max frame"]);
        widths.extend([
            Constraint::Length(6),
            Constraint::Length(6),
            Constraint::Length(16),
        ]);
    }
    headings.push(if status.resources.shared_pool {
        "Active"
    } else {
        "CPU/budget"
    });
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
            .header(Row::new(headings).style(accent).bottom_margin(1))
            .column_spacing(1),
        areas[2],
    );
    let hidden = status.jobs.len().saturating_sub(capacity);
    let footer = status.stop_reason.as_deref().map(clean).unwrap_or_else(|| {
        format!(
            "{} hidden parents · CP = completed-sector checkpoint; active sectors restart",
            hidden,
        )
    });
    let quiet = if color {
        Style::default().fg(Color::Gray)
    } else {
        Style::default()
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(footer, quiet)),
            Line::from("Rules+/Resid+ = new this invocation, not an independent master count."),
            Line::from("Read-only; campaign signals still stop work."),
        ])
        .wrap(Wrap { trim: false }),
        areas[3],
    );
}

fn parent_label(job: &Job) -> String {
    let id = clean(&job.id);
    let short: String = id
        .strip_prefix("topology-")
        .unwrap_or(&id)
        .chars()
        .take(8)
        .collect();
    format!("P{:02} · {short}", job.metadata.parent)
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
    fn topology_ids_remain_distinguishable_and_fullscreen_has_borders_and_progress() {
        let mut value = super::super::model::tests::fixture();
        value["jobs"][0]["id"] = "topology-05373c99ef72fce4".into();
        let mut other = value["jobs"][0].clone();
        other["id"] = "topology-28bbd9922728f59c".into();
        other["metadata"]["parent"] = 4.into();
        value["jobs"].as_array_mut().unwrap().push(other);
        let status = Status::parse(&serde_json::to_vec(&value).unwrap()).unwrap();
        let mut terminal = Terminal::new(TestBackend::new(160, 32)).unwrap();
        terminal
            .draw(|frame| render(frame, &status, 101.0, true))
            .unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        for expected in [
            "╭",
            "╰",
            "PARENTS",
            "0 / 2",
            "05373c99",
            "28bbd992",
            "Host available",
            "RAM / ceiling",
            "Rules+",
            "CP new/reuse",
        ] {
            assert!(text.contains(expected), "missing {expected:?}");
        }
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
