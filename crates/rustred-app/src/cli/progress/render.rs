use std::time::Duration;

use tabled::builder::Builder;
use tabled::settings::Style;

use super::model::DashboardState;

const VALUE_WIDTH: usize = 18;
pub(super) const DASHBOARD_HEIGHT: u16 = 8;
pub(super) const MIN_FULL_TABLE_COLUMNS: u16 = 80;

pub(super) fn render_dashboard(state: &DashboardState) -> String {
    let phase = fixed_left(state.phase.label(), VALUE_WIDTH);
    let reports = match state.task_report_ceiling {
        Some(limit) => format!("{} / {limit}", state.task_reports),
        None => format!("{} / —", state.task_reports),
    };
    let dimension = match (state.effective_dimension, state.maximum_dimension) {
        (Some(effective), Some(maximum)) => format!("{effective} / {maximum}"),
        _ => "—".to_owned(),
    };
    let outcomes = format!(
        "{} / {} / {}",
        state.no_proposal, state.duplicate, state.errors
    );
    let rate = state
        .owner_rate
        .map(format_owner_rate)
        .unwrap_or_else(|| "—".to_owned());
    let eta = state
        .cap_eta
        .map(format_duration)
        .unwrap_or_else(|| "—".to_owned());
    let rss = state
        .rss_bytes
        .map(format_bytes)
        .unwrap_or_else(|| "—".to_owned());

    let mut builder = Builder::with_capacity(6, 4);
    builder.push_record([
        "phase / stop".to_owned(),
        phase,
        "elapsed".to_owned(),
        fixed_right(&format_duration(state.elapsed), VALUE_WIDTH),
    ]);
    builder.push_record([
        "revision".to_owned(),
        fixed_right(&state.revision.to_string(), VALUE_WIDTH),
        "owners".to_owned(),
        fixed_right(&state.owner_count.to_string(), VALUE_WIDTH),
    ]);
    builder.push_record([
        "uncovered boxes".to_owned(),
        fixed_right(
            &state
                .uncovered_box_count
                .map(|count| count.to_string())
                .unwrap_or_else(|| "—".to_owned()),
            VALUE_WIDTH,
        ),
        "dim eff / max".to_owned(),
        fixed_right(&dimension, VALUE_WIDTH),
    ]);
    match state.wave {
        None => builder.push_record([
            "strict shrink".to_owned(),
            fixed_right(&state.strict_shrink.to_string(), VALUE_WIDTH),
            "no / dup / err".to_owned(),
            fixed_right(&outcomes, VALUE_WIDTH),
        ]),
        Some(wave) => builder.push_record([
            "wave / rank".to_owned(),
            fixed_right(
                &format!("{} / {}", wave.ordinal.saturating_add(1), wave.active_count),
                VALUE_WIDTH,
            ),
            "orbits c / r / t".to_owned(),
            fixed_right(
                &format!(
                    "{} / {} / {}",
                    wave.closed_orbit_count, wave.running_orbit_count, wave.terminal_orbit_count
                ),
                VALUE_WIDTH,
            ),
        ]),
    }
    builder.push_record([
        "owner rate".to_owned(),
        fixed_right(&rate, VALUE_WIDTH),
        "cap ETA".to_owned(),
        fixed_right(&eta, VALUE_WIDTH),
    ]);
    builder.push_record([
        "RSS".to_owned(),
        fixed_right(&rss, VALUE_WIDTH),
        "reports / cap".to_owned(),
        fixed_right(&reports, VALUE_WIDTH),
    ]);
    let mut table = builder.build();
    table.with(Style::modern_rounded());
    table.to_string()
}

/// Width-adaptive content for Ratatui's clipped inline viewport.
pub(super) fn render_dashboard_for_width(state: &DashboardState, width: u16) -> String {
    if width >= MIN_FULL_TABLE_COLUMNS {
        return render_dashboard(state);
    }
    let cap = state
        .task_report_ceiling
        .map(|value| value.to_string())
        .unwrap_or_else(|| "—".to_owned());
    let boxes = state
        .uncovered_box_count
        .map(|value| value.to_string())
        .unwrap_or_else(|| "—".to_owned());
    let dimension = match (state.effective_dimension, state.maximum_dimension) {
        (Some(effective), Some(maximum)) => format!("{effective}/{maximum}"),
        _ => "—".to_owned(),
    };
    let rss = state
        .rss_bytes
        .map(format_bytes)
        .unwrap_or_else(|| "—".to_owned());
    let scope = match state.wave {
        None => format!("dim {dimension} · RSS {rss}"),
        Some(wave) => format!(
            "wave {} · rank {} · orbits {}/{}",
            wave.ordinal.saturating_add(1),
            wave.active_count,
            wave.closed_orbit_count,
            wave.orbit_count
        ),
    };
    let outcomes = match state.wave {
        None => format!(
            "shrink {} · no/dup/err {}/{}/{}",
            state.strict_shrink, state.no_proposal, state.duplicate, state.errors
        ),
        Some(wave) => format!(
            "running {} · terminal {} · RSS {rss}",
            wave.running_orbit_count, wave.terminal_orbit_count
        ),
    };
    [
        format!(
            "RustRed · {} · {}",
            state.phase.label(),
            format_duration(state.elapsed)
        ),
        format!("rev {} · owners {}", state.revision, state.owner_count),
        format!("reports {}/{cap} · boxes {boxes}", state.task_reports),
        scope,
        outcomes,
    ]
    .join("\n")
}

fn fixed_left(value: &str, width: usize) -> String {
    let value = clip(value, width);
    format!("{value:<width$}")
}

fn fixed_right(value: &str, width: usize) -> String {
    let value = clip(value, width);
    format!("{value:>width$}")
}

fn clip(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        return value.to_owned();
    }
    let retained = value
        .chars()
        .rev()
        .take(width.saturating_sub(1))
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!(">{retained}")
}

fn format_owner_rate(rate: f64) -> String {
    if rate < 100.0 {
        format!("{rate:.2} owners/s")
    } else if rate < 10_000.0 {
        format!("{rate:.1} owners/s")
    } else {
        format!("{rate:.0} owners/s")
    }
}

fn format_duration(duration: Duration) -> String {
    let seconds = duration.as_secs();
    let hours = seconds / 3_600;
    let minutes = seconds / 60 % 60;
    let seconds = seconds % 60;
    format!("{hours:02}:{minutes:02}:{seconds:02}")
}

fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1_024.0;
    const MIB: f64 = KIB * KIB;
    const GIB: f64 = MIB * KIB;
    let bytes = bytes as f64;
    if bytes >= GIB {
        format!("{:.2} GiB", bytes / GIB)
    } else if bytes >= MIB {
        format!("{:.1} MiB", bytes / MIB)
    } else if bytes >= KIB {
        format!("{:.1} KiB", bytes / KIB)
    } else {
        format!("{bytes:.0} B")
    }
}
