//! Screens. Everything is composed as lines of styled spans with fixed-width columns (deterministic golden
//! screens, exact control of which columns are dropped when narrow; nothing ever wraps), then drawn as one paragraph.
//! Tables are windowed: only visible rows are built.
//!
//! Rendering pattern (TestBackend golden screens, a `screen()` text dump) follows rustred-app
//! `cli/shards/monitor.rs` (b9f06de7); layouts follow the plan's mock-ups of 3.U.2.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::app::{App, CompactTable, Panel};
use super::format::{
    cell, cell_u, clean, count, countdown, cpu_hours, dur, fit, gib, hms, percent, rough, thousands,
};
use super::model::{
    AttentionRow, Banner, Coverage, EventRow, EventTag, LadderRow, NodeRow, PHASES, Severity,
    ViewModel, phase_is_open,
};
use super::source::JobRow;
use super::theme::{Role, Theme};

/// Width class of the summary screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WidthClass {
    Tiny,
    Compact,
    Default,
    Wide,
}

impl WidthClass {
    pub fn of(width: u16, height: u16) -> Self {
        if width < 80 || height < 24 {
            WidthClass::Tiny
        } else if width < 120 {
            WidthClass::Compact
        } else if width < 200 {
            WidthClass::Default
        } else {
            WidthClass::Wide
        }
    }
}

/// Everything a frame needs besides the view-model.
pub struct Ctx<'a> {
    pub theme: Theme,
    pub utf8: bool,
    pub app: &'a App,
    pub now_s: f64,
}

type Cells = Vec<(String, Role)>;

fn span(s: impl Into<String>, r: Role, t: Theme) -> Span<'static> {
    Span::styled(s.into(), t.style(r))
}

fn line_width(l: &Line) -> usize {
    l.spans.iter().map(|s| s.content.chars().count()).sum()
}

/// Pads (or cuts) a line to exactly `w` cells.
fn pad_line(mut l: Line<'static>, w: usize) -> Line<'static> {
    let mut used = 0usize;
    let mut out: Vec<Span<'static>> = Vec::new();
    for s in l.spans.drain(..) {
        let n = s.content.chars().count();
        if used + n <= w {
            used += n;
            out.push(s);
        } else {
            let keep: String = s.content.chars().take(w - used).collect();
            used += keep.chars().count();
            out.push(Span::styled(keep, s.style));
            break;
        }
    }
    if used < w {
        out.push(Span::raw(" ".repeat(w - used)));
    }
    Line::from(out)
}

/// Joins two columns of lines side by side.
fn hjoin(
    left: Vec<Line<'static>>,
    lw: usize,
    right: Vec<Line<'static>>,
    gap: usize,
) -> Vec<Line<'static>> {
    let n = left.len().max(right.len());
    let mut l = left.into_iter();
    let mut r = right.into_iter();
    (0..n)
        .map(|_| {
            let a = pad_line(l.next().unwrap_or_default(), lw);
            let mut spans = a.spans;
            spans.push(Span::raw(" ".repeat(gap)));
            if let Some(b) = r.next() {
                spans.extend(b.spans);
            }
            Line::from(spans)
        })
        .collect()
}

/// A section rule: `-[ title ]-----` (ASCII) or `─[ title ]─────` (UTF-8).
fn rule(title: &str, hint: &str, w: usize, c: &Ctx) -> Line<'static> {
    let ch = if c.utf8 { '─' } else { '-' };
    let head = format!("{ch}[ {title} ]");
    let tail = if hint.is_empty() {
        String::new()
    } else {
        format!(" {hint} {ch}{ch}{ch}{ch}")
    };
    let fill = w.saturating_sub(head.chars().count() + tail.chars().count());
    let mut s = head;
    s.extend(std::iter::repeat_n(ch, fill));
    s.push_str(&tail);
    Line::from(span(fit(&s, w), Role::Title, c.theme))
}

/// Adds segments (separated by `sep` spaces) while they fit into `w`; segments are in priority order.
fn segments(segs: Vec<Vec<Span<'static>>>, w: usize, sep: usize) -> Line<'static> {
    let mut out: Vec<Span<'static>> = Vec::new();
    let mut used = 0usize;
    for seg in segs {
        let n: usize = seg.iter().map(|s| s.content.chars().count()).sum();
        if n == 0 {
            continue;
        }
        let extra = if out.is_empty() { 0 } else { sep };
        if used + extra + n > w {
            continue;
        }
        if extra > 0 {
            out.push(Span::raw(" ".repeat(extra)));
        }
        used += extra + n;
        out.extend(seg);
    }
    Line::from(out)
}

#[derive(Clone, Copy)]
enum Align {
    L,
    R,
}

/// A table column: title, width, alignment, drop priority (higher = dropped first when narrow).
struct Col {
    title: &'static str,
    width: usize,
    align: Align,
    drop: u8,
}

const fn col(title: &'static str, width: usize, align: Align, drop: u8) -> Col {
    Col {
        title,
        width,
        align,
        drop,
    }
}

/// Which columns fit into `w` (dropping by priority, never wrapping).
fn fit_cols(cols: &[Col], w: usize) -> Vec<usize> {
    let mut keep: Vec<usize> = (0..cols.len()).filter(|&i| cols[i].drop < 99).collect();
    let total = |k: &[usize]| -> usize {
        k.iter().map(|&i| cols[i].width).sum::<usize>() + k.len().saturating_sub(1)
    };
    while total(&keep) > w && keep.len() > 1 {
        let (pos, _) = keep
            .iter()
            .enumerate()
            .max_by_key(|(p, i)| (cols[**i].drop, *p))
            .expect("non-empty");
        if cols[keep[pos]].drop == 0 {
            break;
        }
        keep.remove(pos);
    }
    keep
}

fn cell_text(s: &str, c: &Col) -> String {
    let s = fit(s, c.width);
    match c.align {
        Align::L => format!("{s:<w$}", w = c.width),
        Align::R => format!("{s:>w$}", w = c.width),
    }
}

/// Renders a table: header + rows (already windowed by the caller); `selected` row is reversed.
fn table(
    cols: &[Col],
    rows: &[Cells],
    w: usize,
    selected: Option<usize>,
    c: &Ctx,
) -> Vec<Line<'static>> {
    let keep = fit_cols(cols, w);
    let mut out = Vec::with_capacity(rows.len() + 1);
    let head: Vec<Span<'static>> = keep
        .iter()
        .enumerate()
        .flat_map(|(k, &i)| {
            let mut v = Vec::new();
            if k > 0 {
                v.push(Span::raw(" "));
            }
            v.push(span(cell_text(cols[i].title, &cols[i]), Role::Label, c.theme));
            v
        })
        .collect();
    out.push(Line::from(head));
    for (ri, row) in rows.iter().enumerate() {
        let sel = selected == Some(ri);
        let mut spans: Vec<Span<'static>> = Vec::new();
        for (k, &i) in keep.iter().enumerate() {
            if k > 0 {
                spans.push(Span::raw(" "));
            }
            let (text, role) = row.get(i).cloned().unwrap_or_default();
            let mut st = c.theme.style(role);
            if sel {
                st = st.patch(c.theme.style(Role::Selected));
            }
            spans.push(Span::styled(cell_text(&text, &cols[i]), st));
        }
        out.push(Line::from(spans));
    }
    out
}

/// Window of `n` rows of `total` that keeps `sel` visible.
fn window(total: usize, n: usize, sel: usize) -> (usize, usize) {
    if total <= n {
        return (0, total);
    }
    let start = sel.saturating_sub(n.saturating_sub(1)).min(total - n);
    (start, start + n)
}

// ------------------------------------------------------------------------------------------------------------
// roles of values
// ------------------------------------------------------------------------------------------------------------

fn cov_role(n: &NodeRow) -> Role {
    match n.coverage {
        Coverage::Full => Role::Ok,
        Coverage::Boxed => Role::Boxed,
        Coverage::Open if n.phase == "open" => Role::Fail,
        Coverage::Open if n.phase == "stale" => Role::Warn,
        Coverage::Open if matches!(n.phase.as_str(), "running" | "closure_check") => Role::Running,
        _ => Role::Queued,
    }
}

fn cov_text(n: &NodeRow) -> String {
    match n.coverage {
        Coverage::Boxed => format!("[BOXED {}]", n.target),
        Coverage::Full => "FULL".into(),
        Coverage::Open => "OPEN".into(),
        Coverage::Unknown => "?".into(),
    }
}

fn tag_role(t: EventTag) -> Role {
    match t {
        EventTag::Ok => Role::Ok,
        EventTag::Esc => Role::Warn,
        EventTag::Fail => Role::Fail,
        EventTag::Term => Role::Terminal,
        EventTag::Boxed => Role::Boxed,
        EventTag::Info => Role::Normal,
    }
}

fn banner_role(b: Banner) -> Role {
    match b {
        Banner::Running | Banner::Finished => Role::Ok,
        Banner::Starting => Role::Running,
        Banner::Draining | Banner::Stale | Banner::Unknown => Role::Warn,
        Banner::Stopped => Role::Label,
        Banner::NoData | Banner::Unreadable => Role::Fail,
    }
}

/// Role of a used / cap pair: warn above 90 %.
fn cap_role(used: Option<u64>, cap: Option<u64>) -> Role {
    match (used, cap) {
        (Some(u), Some(c)) if c > 0 && u * 10 > c * 9 => Role::Warn,
        _ => Role::Normal,
    }
}

fn host_role(avail: Option<u64>, floor: Option<u64>) -> Role {
    match (avail, floor) {
        (Some(a), Some(f)) if a < f => Role::Fail,
        (Some(a), Some(f)) if a < f + f / 4 => Role::Warn,
        _ => Role::Normal,
    }
}

// ------------------------------------------------------------------------------------------------------------
// header block
// ------------------------------------------------------------------------------------------------------------

/// The ETA sentence (a range, never one number) and its role.
pub fn eta_text(vm: &ViewModel) -> String {
    let e = &vm.eta;
    if vm.seq.is_none() {
        return "ETA ?".into();
    }
    if e.remaining_items == Some(0) {
        return "ETA: nothing left".into();
    }
    let items = e
        .remaining_items
        .map(|n| format!("; {} items", thousands(n)))
        .unwrap_or_default();
    let jobs = e
        .parallelism
        .map(|n| format!("; {n} jobs"))
        .unwrap_or_default();
    let pooled = if e.pooled_levels.is_empty() {
        String::new()
    } else {
        format!("; pooled {}", e.pooled_levels.join(","))
    };
    match (e.wall_s, e.cpu_s) {
        (Some((lo, hi)), Some((clo, chi))) => format!(
            "ETA ~ {}-{} wall ({}-{} CPU left{items}{jobs}{pooled})",
            rough(lo),
            rough(hi),
            rough(clo),
            rough(chi)
        ),
        (None, Some((clo, chi))) => format!(
            "ETA ~ ? wall ({}-{} CPU left{items}; {})",
            rough(clo),
            rough(chi),
            e.reason.clone().unwrap_or_else(|| "no parallelism".into())
        ),
        _ => format!(
            "ETA: too little data ({})",
            e.reason.clone().unwrap_or_else(|| "no estimate".into())
        ),
    }
}

fn kv(label: &str, value: String, vr: Role, t: Theme) -> Vec<Span<'static>> {
    vec![span(format!("{label} "), Role::Label, t), span(value, vr, t)]
}

/// Header line 1: identity, state, ages.
fn header_line(vm: &ViewModel, w: usize, c: &Ctx) -> Line<'static> {
    let t = c.theme;
    let state_word = if c.app.stop_count >= 2 {
        "STOPPING (forced)".to_string()
    } else if c.app.stop_count == 1 {
        format!(
            "STOPPING {}",
            countdown(c.app.grace_left(vm, c.now_s).unwrap_or(0.0))
        )
    } else {
        vm.banner.word().to_string()
    };
    let srole = if c.app.stop_count > 0 {
        Role::Warn
    } else {
        banner_role(vm.banner)
    };
    let mut segs = vec![
        vec![span("rr_campaign", Role::Title, t)],
        vec![span(
            vm.campaign_id.clone().unwrap_or_else(|| "?".into()),
            Role::Label,
            t,
        )],
        vec![span(state_word, srole, t)],
        kv(vm.elapsed_label, dur(vm.elapsed_s), Role::Normal, t),
        kv(
            "hb",
            dur(vm.heartbeat_age_s),
            if vm.banner == Banner::Stale {
                Role::Warn
            } else {
                Role::Normal
            },
            t,
        ),
        kv("seq", count(vm.seq), Role::Normal, t),
    ];
    if let Some(s) = vm.sessions {
        segs.push(kv("sessions", s.to_string(), Role::Normal, t));
    }
    let rec = vm.crash_recoveries.unwrap_or(0)
        + vm.lock_takeovers.unwrap_or(0)
        + vm.torn_tail_recoveries.unwrap_or(0);
    if rec > 0 {
        segs.push(kv("recoveries", rec.to_string(), Role::Warn, t));
    }
    if let Some(ti) = &vm.title {
        segs.push(vec![span(ti.clone(), Role::Normal, t)]);
    }
    segs.push(kv("dir", vm.origin.clone(), Role::Queued, t));
    segments(segs, w, 2)
}

fn resources_line(vm: &ViewModel, w: usize, c: &Ctx) -> Line<'static> {
    let t = c.theme;
    let r = &vm.resources;
    let mut segs = vec![
        kv(
            "CPU",
            format!("{}/{}", count(r.cpus_in_use), count(r.cpus_allowed)),
            cap_role(r.cpus_in_use, r.cpus_allowed),
            t,
        ),
        kv(
            "RAM",
            format!(
                "{} used, {} res / {}G cap",
                gib(r.rss_bytes),
                gib(r.reserved_bytes),
                gib(r.mem_cap_bytes)
            ),
            cap_role(r.rss_bytes.max(r.reserved_bytes), r.mem_cap_bytes),
            t,
        ),
        kv(
            "host",
            format!(
                "{}G (floor {})",
                gib(r.host_available_bytes),
                gib(r.host_floor_bytes)
            ),
            host_role(r.host_available_bytes, r.host_floor_bytes),
            t,
        ),
        kv(
            "jobs",
            format!(
                "{}{}",
                count(vm.jobs.running),
                vm.jobs
                    .cap
                    .map(|c| format!("/{c}"))
                    .unwrap_or_default()
            ),
            Role::Running,
            t,
        ),
    ];
    if let Some(d) = r.disk_free_bytes {
        segs.push(kv("disk", super::format::bytes(Some(d)), Role::Normal, t));
    }
    if let Some(l) = vm.jobs.launching.filter(|n| *n > 0) {
        segs.push(kv("launching", l.to_string(), Role::Running, t));
    }
    segments(segs, w, 3)
}

fn nodes_line(vm: &ViewModel, w: usize, c: &Ctx) -> Line<'static> {
    let t = c.theme;
    let mut segs = vec![
        kv("NODES", count(vm.nodes_total), Role::Normal, t),
        kv("FULL", count(vm.nodes_full), Role::Ok, t),
        kv("BOXED", count(vm.nodes_boxed), Role::Boxed, t),
        kv(
            "OPEN",
            count(vm.nodes_open),
            if vm.nodes_open.unwrap_or(0) > 0 {
                Role::Warn
            } else {
                Role::Normal
            },
            t,
        ),
        kv("certified", count(vm.nodes_certified), Role::Ok, t),
    ];
    if vm.faces_total.is_some() {
        segs.push(kv(
            "faces",
            format!("{}/{}", count(vm.faces_closed), count(vm.faces_total)),
            Role::Normal,
            t,
        ));
    }
    let phases: Vec<String> = vm
        .nodes_by_phase
        .iter()
        .filter(|(_, n)| **n > 0)
        .map(|(p, n)| format!("{p}={n}"))
        .collect();
    if !phases.is_empty() {
        segs.push(vec![span(
            format!("({})", phases.join(", ")),
            Role::Queued,
            t,
        )]);
    }
    let att = vm.attention.len();
    segs.insert(
        5,
        kv(
            "attention",
            att.to_string(),
            if att > 0 { Role::Warn } else { Role::Normal },
            t,
        ),
    );
    segments(segs, w, 2)
}

fn items_line(vm: &ViewModel, w: usize, c: &Ctx) -> Line<'static> {
    let t = c.theme;
    let ph = |p: &str| vm.items_by_phase[PHASES.iter().position(|x| *x == p).unwrap_or(0)];
    let segs = vec![
        kv("ITEMS", count(vm.items_total), Role::Normal, t),
        kv("open", count(vm.items_open), Role::Normal, t),
        kv("run", count(ph("in_progress")), Role::Running, t),
        kv("acc", count(ph("accepted")), Role::Ok, t),
        kv("esc", count(ph("escalated")), Role::Warn, t),
        kv("rej", count(ph("rejected")), Role::Fail, t),
        kv("term", count(ph("terminal")), Role::Terminal, t),
        kv("ret", count(ph("retired")), Role::Queued, t),
        kv("stale", count(ph("stale")), Role::Warn, t),
    ];
    segments(segs, w, 2)
}

fn rules_line(vm: &ViewModel, w: usize, c: &Ctx) -> Line<'static> {
    let t = c.theme;
    let r = &vm.rules;
    let segs = vec![
        kv("RULES accepted", count(r.accepted), Role::Ok, t),
        kv(
            "rejected",
            count(r.rejected),
            if r.rejected.unwrap_or(0) > 0 {
                Role::Fail
            } else {
                Role::Normal
            },
            t,
        ),
        kv(
            "CERT",
            format!(
                "{} ({}/{} proved)",
                percent(vm.certified_share),
                count(vm.certified_proved),
                count(vm.certified_accepted)
            ),
            Role::Ok,
            t,
        ),
        kv("upgrades", count(r.upgrades), Role::Normal, t),
        kv("retired", count(r.retired), Role::Queued, t),
    ];
    segments(segs, w, 2)
}

fn closure_line(vm: &ViewModel, w: usize, c: &Ctx) -> Line<'static> {
    let t = c.theme;
    let r = &vm.rules;
    let defect = match vm.defect_keys {
        None => "not checked yet".to_string(),
        Some(d) => format!(
            "{} keys ({} of {} nodes with defect)",
            thousands(d),
            count(vm.nodes_with_defect),
            count(vm.nodes_checked)
        ),
    };
    let segs = vec![
        kv(
            "TERMINALS",
            format!(
                "{} (cand {}, bound {}/node)",
                count(vm.terminals_declared),
                count(vm.terminal_candidates),
                count(vm.terminal_bound)
            ),
            Role::Terminal,
            t,
        ),
        kv(
            "DEFECT",
            defect,
            match vm.defect_keys {
                Some(0) => Role::Ok,
                Some(_) => Role::Fail,
                None => Role::Estimate,
            },
            t,
        ),
        kv(
            "ECONOMY rules",
            format!("{}->{}", count(r.rules_before), count(r.rules_after)),
            Role::Normal,
            t,
        ),
        kv(
            "RHS",
            format!("{}->{}", count(r.rhs_before), count(r.rhs_after)),
            Role::Normal,
            t,
        ),
    ];
    segments(segs, w, 2)
}

fn eta_line(vm: &ViewModel, w: usize, c: &Ctx) -> Line<'static> {
    let t = c.theme;
    let used = kv(
        "cpu-h",
        format!(
            "{}{}",
            cpu_hours(vm.cost_cpu_s),
            vm.budget_cpu_hours
                .map(|b| format!("/{b:.0}"))
                .unwrap_or_default()
        ),
        Role::Normal,
        t,
    );
    let n: usize = used.iter().map(|s| s.content.chars().count()).sum();
    let mut spans = used;
    spans.push(Span::raw("  "));
    spans.push(span(fit(&eta_text(vm), w.saturating_sub(n + 2)), Role::Estimate, t));
    Line::from(spans)
}

/// Banner lines: stop protocol, STALE, problems, transient message.
fn banners(vm: &ViewModel, w: usize, c: &Ctx) -> Vec<Line<'static>> {
    let t = c.theme;
    let mut out = Vec::new();
    let running = vm.jobs.running.unwrap_or(0) + vm.jobs.launching.unwrap_or(0);
    if c.app.stop_count >= 2 {
        out.push(Line::from(span(
            fit(
                &format!(
                    " FORCED STOP: {running} running job(s) are being killed and recorded as interrupted; still safe (only sealed outputs are accepted). Resume with the same command. "
                ),
                w,
            ),
            Role::BannerFail,
            t,
        )));
    } else if c.app.stop_count == 1 {
        let left = c.app.grace_left(vm, c.now_s).unwrap_or(0.0);
        out.push(Line::from(span(
            fit(
                &format!(
                    " STOPPING {}  draining: {running} job(s) running, grace {} s; nothing half-accepted. Press Ctrl-C again to force. ",
                    countdown(left),
                    c.app.grace_s
                ),
                w,
            ),
            Role::BannerWarn,
            t,
        )));
    }
    if vm.banner == Banner::Stale {
        out.push(Line::from(span(
            fit(
                &format!(
                    " STALE: no heartbeat for {} (> {} s): supervisor not running? Resume with the same command. ",
                    dur(vm.heartbeat_age_s),
                    vm.stale_after_s as u64
                ),
                w,
            ),
            Role::BannerWarn,
            t,
        )));
    }
    if matches!(vm.banner, Banner::NoData | Banner::Unreadable) || !vm.problems.is_empty() {
        let more = if vm.problems.len() > 1 {
            format!(" (+{} more)", vm.problems.len() - 1)
        } else {
            String::new()
        };
        let first = vm.problems.first().cloned().unwrap_or_default();
        out.push(Line::from(span(
            fit(&format!("! {first}{more}"), w),
            Role::Fail,
            t,
        )));
    }
    if let Some(m) = c.app.message_at(c.now_s) {
        out.push(Line::from(span(fit(&format!("> {m}"), w), Role::Warn, t)));
    }
    out
}

fn footer(vm: &ViewModel, w: usize, class: WidthClass, c: &Ctx) -> Line<'static> {
    let att = vm.attention.len();
    let stop = if c.app.hosts_campaign {
        "Ctrl-C stop safely (twice: now)"
    } else {
        "q/Ctrl-C close viewer (campaign unaffected)"
    };
    let s = match (c.app.panel, class) {
        (Panel::Summary, WidthClass::Compact) => {
            format!("Tab table  n/N node  Enter detail  a attention({att})  ?help  {stop}")
        }
        (Panel::Summary, _) => format!(
            "n/N select node  Enter node detail  j jobs  e events  a attention({att})  ?help  {stop}"
        ),
        (Panel::Node, _) => format!(
            "n/N item  Enter/w why  1-9 filter by state (0 all)  PgUp/PgDn  q back  {stop}"
        ),
        _ => format!("arrows/PgUp/PgDn scroll  q back  {stop}"),
    };
    Line::from(span(fit(&s, w), Role::Queued, c.theme))
}

// ------------------------------------------------------------------------------------------------------------
// tables
// ------------------------------------------------------------------------------------------------------------

fn nodes_table(vm: &ViewModel, w: usize, rows: usize, class: WidthClass, c: &Ctx) -> Vec<Line<'static>> {
    let compact = class == WidthClass::Compact;
    let has_faces = vm.nodes.iter().any(|n| n.faces_total.is_some());
    let cols = [
        col("node", 8, Align::L, 0),
        col("coverage", 19, Align::L, 0),
        col("cert", 4, Align::L, 8),
        col("phase", 13, Align::L, 3),
        col("faces ok/all", 12, Align::R, if !has_faces { 99 } else if compact { 9 } else { 5 }),
        col("open", 6, Align::R, 0),
        col("by rung", 18, Align::L, if compact { 9 } else { 5 }),
        col("term", 7, Align::R, 1),
        col("defect", 8, Align::R, 1),
        col("cert%", 6, Align::R, if compact { 9 } else { 4 }),
        col("rules", 7, Align::R, 2),
        col("used%", 6, Align::R, 2),
        col("RHS", 9, Align::R, if compact { 9 } else { 6 }),
        col("CPU-h", 7, Align::R, if compact { 9 } else { 6 }),
    ];
    let total = vm.nodes.len();
    let sel = c.app.selected_node.min(total.saturating_sub(1));
    let n = rows.saturating_sub(1).max(1);
    let (lo, hi) = window(total, if total > n { n.saturating_sub(1).max(1) } else { n }, sel);
    let data: Vec<Cells> = vm.nodes[lo..hi]
        .iter()
        .map(|nd| {
            let rr = cov_role(nd);
            let row_role = if nd.coverage == Coverage::Boxed { Role::Boxed } else { Role::Normal };
            let by_rung = nd
                .open_by_rung
                .iter()
                .map(|(r, k)| format!("{r} {k}"))
                .collect::<Vec<_>>()
                .join(" ");
            let term = match (nd.terminals, nd.terminal_bound) {
                (Some(t), Some(b)) => format!("{t}/{b}"),
                (Some(t), None) => t.to_string(),
                _ => "-".into(),
            };
            vec![
                (format!("{}{}", if nd.stale_reason.is_some() { "!" } else { "" }, nd.id), row_role),
                (cov_text(nd), rr),
                (if nd.certified { "yes".into() } else { "-".into() }, if nd.certified { Role::Ok } else { row_role }),
                (if nd.problem.is_some() { "unreadable".into() } else { nd.phase.clone() }, if nd.problem.is_some() { Role::Fail } else { row_role }),
                (
                    match (nd.faces_closed, nd.faces_total) {
                        (Some(a), Some(b)) => format!("{a}/{b}"),
                        _ => "-".into(),
                    },
                    row_role,
                ),
                (cell(nd.items_open), row_role),
                (if by_rung.is_empty() { "-".into() } else { by_rung }, row_role),
                (term, Role::Terminal),
                (
                    match nd.defect_keys {
                        None => "-".into(),
                        Some(d) => thousands(d),
                    },
                    match nd.defect_keys {
                        Some(0) => Role::Ok,
                        Some(_) => Role::Fail,
                        None => Role::Estimate,
                    },
                ),
                (percent(nd.certified_share), row_role),
                (cell(nd.rules_after), row_role),
                (percent(nd.used_share), row_role),
                (cell(nd.rhs_after), row_role),
                (cpu_hours(nd.cpu_seconds), row_role),
            ]
        })
        .collect();
    let mut out = table(&cols, &data, w, Some(sel - lo), c);
    if hi - lo < total {
        out.push(Line::from(span(
            format!("... {} more (n/N to scroll; {} of {total})", total - (hi - lo), sel + 1),
            Role::Queued,
            c.theme,
        )));
    }
    if total == 0 {
        out.push(Line::from(span("(no node files yet)", Role::Queued, c.theme)));
    }
    out
}

fn ladder_table(vm: &ViewModel, w: usize, c: &Ctx) -> Vec<Line<'static>> {
    let cols = [
        col("rung", 4, Align::L, 0),
        col("open", 6, Align::R, 0),
        col("run", 4, Align::R, 0),
        col("prod", 4, Align::R, 3),
        col("acc", 6, Align::R, 0),
        col("rej", 4, Align::R, 1),
        col("esc", 4, Align::R, 1),
        col("tcand", 5, Align::R, 4),
        col("term", 4, Align::R, 1),
        col("stale", 5, Align::R, 4),
        col("ret", 4, Align::R, 4),
        col("jobs", 5, Align::R, 2),
        col("med s", 6, Align::R, 2),
    ];
    let role_of = |p: &str| match p {
        "in_progress" => Role::Running,
        "accepted" => Role::Ok,
        "rejected" => Role::Fail,
        "escalated" | "stale" | "terminal_candidate" => Role::Warn,
        "terminal" => Role::Terminal,
        "retired" => Role::Queued,
        _ => Role::Normal,
    };
    let rows: Vec<Cells> = vm
        .ladder
        .iter()
        .map(|l: &LadderRow| {
            let mut v: Cells = vec![(l.rung.clone(), Role::Label)];
            v.push((cell_u(l.open()), Role::Normal));
            for p in [
                "in_progress",
                "produced",
                "accepted",
                "rejected",
                "escalated",
                "terminal_candidate",
                "terminal",
                "stale",
                "retired",
            ] {
                let k = l.counts[PHASES.iter().position(|x| *x == p).unwrap_or(0)];
                v.push((
                    if k == 0 { "-".into() } else { thousands(k) },
                    if k == 0 { Role::Queued } else { role_of(p) },
                ));
            }
            v.push((cell(l.jobs), Role::Normal));
            v.push((
                l.median_s
                    .map(|m| format!("{m:.1}"))
                    .unwrap_or_else(|| "-".into()),
                Role::Estimate,
            ));
            v
        })
        .collect();
    let mut out = table(&cols, &rows, w, None, c);
    if rows.is_empty() {
        out.push(Line::from(span("(no items yet)", Role::Queued, c.theme)));
    }
    out
}

fn rules_panel(vm: &ViewModel, w: usize, c: &Ctx) -> Vec<Line<'static>> {
    let t = c.theme;
    let r = &vm.rules;
    let mut out = vec![segments(
        vec![
            kv("accepted", count(r.accepted), Role::Ok, t),
            kv("rejected", count(r.rejected), Role::Fail, t),
            kv("upgrades", count(r.upgrades), Role::Normal, t),
            kv("retired", count(r.retired), Role::Queued, t),
        ],
        w,
        2,
    )];
    let reasons: Vec<String> = r
        .rejected_by_reason
        .iter()
        .map(|(k, v)| format!("{k} {v}"))
        .collect();
    out.push(Line::from(span(
        fit(
            &format!(
                "rejected by reason: {}",
                if reasons.is_empty() { "-".into() } else { reasons.join(", ") }
            ),
            w,
        ),
        Role::Normal,
        t,
    )));
    out.push(Line::from(span(
        fit(
            &format!(
                "evidence: geometry {}  tested {}  proved {}",
                count(r.by_evidence[0]),
                count(r.by_evidence[1]),
                count(r.by_evidence[2])
            ),
            w,
        ),
        Role::Normal,
        t,
    )));
    out.push(Line::from(span(
        fit(
            &format!(
                "certified share {} ({})",
                percent(vm.certified_share),
                vm.certified_basis.clone().unwrap_or_else(|| "-".into())
            ),
            w,
        ),
        Role::Ok,
        t,
    )));
    out.push(Line::from(span(
        fit(
            &format!(
                "economy: rules {} -> {}, RHS terms {} -> {}",
                count(r.rules_before),
                count(r.rules_after),
                count(r.rhs_before),
                count(r.rhs_after)
            ),
            w,
        ),
        Role::Normal,
        t,
    )));
    let defects: Vec<String> = vm
        .nodes
        .iter()
        .filter_map(|n| n.defect_keys.map(|d| format!("{} {}", n.id, thousands(d))))
        .collect();
    out.push(Line::from(span(
        fit(
            &format!(
                "closure defect keys: {}",
                if defects.is_empty() { "not checked yet".into() } else { defects.join(", ") }
            ),
            w,
        ),
        if vm.defect_keys.unwrap_or(0) > 0 { Role::Fail } else { Role::Normal },
        t,
    )));
    let classes: Vec<String> = vm
        .jobs
        .settled_by_class
        .iter()
        .map(|(k, v)| format!("{k} {v}"))
        .collect();
    out.push(Line::from(span(
        fit(
            &format!(
                "jobs {} settled: {}",
                count(vm.jobs.total),
                if classes.is_empty() { "-".into() } else { classes.join(", ") }
            ),
            w,
        ),
        Role::Normal,
        t,
    )));
    out
}

fn jobs_rows(vm: &ViewModel) -> Option<&Vec<JobRow>> {
    vm.jobs.list.as_ref()
}

fn jobs_table(vm: &ViewModel, w: usize, rows: usize, wide: bool, scroll: usize, c: &Ctx) -> Vec<Line<'static>> {
    let cols = [
        col("job", 18, Align::L, 0),
        col("node", 8, Align::L, 0),
        col("item", 18, Align::L, 1),
        col("rung", 4, Align::L, 0),
        col("cpu", 4, Align::R, 3),
        col("RSS / cap", 15, Align::R, 2),
        col("elapsed / budget", 16, Align::R, 2),
        col("hb", 5, Align::R, 4),
        col("phase", 10, Align::L, 3),
        col("pid", 9, Align::R, if wide { 5 } else { 99 }),
        col("run dir", 30, Align::L, if wide { 5 } else { 99 }),
    ];
    let Some(list) = jobs_rows(vm) else {
        return vec![Line::from(span(
            fit(
                &format!(
                    "running {} (job list not in the progress files and not available from the ledger)",
                    count(vm.jobs.running)
                ),
                w,
            ),
            Role::Queued,
            c.theme,
        ))];
    };
    let total = list.len();
    let n = rows.saturating_sub(1).max(1);
    let lo = scroll.min(total.saturating_sub(1));
    let hi = (lo + if total - lo > n { n.saturating_sub(1).max(1) } else { n }).min(total);
    let data: Vec<Cells> = list[lo..hi]
        .iter()
        .map(|j| {
            let rss = match (j.rss_bytes, j.cap_bytes) {
                (None, None) => "-".into(),
                (a, b) => format!("{} / {}", if a.is_some() { gib(a) } else { "-".into() }, if b.is_some() { gib(b) } else { "-".into() }),
            };
            let el = match (j.elapsed_s, j.budget_s) {
                (None, None) => "-".into(),
                (a, b) => format!("{} / {}", dur(a), dur(b)),
            };
            vec![
                (j.job.clone(), Role::Running),
                (j.node.clone(), Role::Normal),
                (j.item.clone(), Role::Normal),
                (j.rung.clone(), Role::Label),
                (cell(j.cpus), Role::Normal),
                (rss, cap_role(j.rss_bytes, j.cap_bytes)),
                (el, Role::Normal),
                (j.heartbeat_age_s.map(|a| dur(Some(a))).unwrap_or_else(|| "-".into()), Role::Normal),
                (j.phase.clone().unwrap_or_else(|| "-".into()), Role::Queued),
                (j.pid.map(|p| p.to_string()).unwrap_or_else(|| "-".into()), Role::Queued),
                (j.run_dir.clone().unwrap_or_else(|| "-".into()), Role::Queued),
            ]
        })
        .collect();
    let mut out = table(&cols, &data, w, None, c);
    if total == 0 {
        out.push(Line::from(span("(no job running)", Role::Queued, c.theme)));
    } else if hi - lo < total {
        out.push(Line::from(span(
            format!("... {} more (j: all jobs)", total - (hi - lo)),
            Role::Queued,
            c.theme,
        )));
    }
    out
}

fn event_lines(events: &[EventRow], w: usize, n: usize, skip: usize, c: &Ctx) -> Vec<Line<'static>> {
    let t = c.theme;
    let mut out: Vec<Line<'static>> = events
        .iter()
        .skip(skip)
        .take(n)
        .map(|e| {
            let head = format!("{} {:<4} ", hms(&e.utc), e.tag.word());
            let rest = format!("{:<17} {}", fit(&e.kind, 17), e.text);
            Line::from(vec![
                span(hms(&e.utc), Role::Queued, t),
                span(" ", Role::Normal, t),
                span(format!("{:<4}", e.tag.word()), tag_role(e.tag), t),
                span(" ", Role::Normal, t),
                span(fit(&rest, w.saturating_sub(head.chars().count())), Role::Normal, t),
            ])
        })
        .collect();
    if events.is_empty() {
        out.push(Line::from(span("(no events yet)", Role::Queued, t)));
    }
    out
}

fn attention_lines(rows: &[AttentionRow], w: usize, n: usize, with_options: bool, c: &Ctx) -> Vec<Line<'static>> {
    let t = c.theme;
    let mut out = Vec::new();
    for a in rows {
        if out.len() >= n {
            break;
        }
        let role = match a.severity {
            Severity::Error => Role::Fail,
            Severity::Decision | Severity::Warning => Role::Warn,
        };
        out.push(Line::from(vec![
            span("! ", role, t),
            span(
                fit(
                    &format!(
                        "{} {} [{}] {}{}",
                        a.id,
                        a.code,
                        a.severity.word(),
                        a.text,
                        if a.derived { " (derived)" } else { "" }
                    ),
                    w.saturating_sub(2),
                ),
                role,
                t,
            ),
        ]));
        if with_options {
            for o in &a.options {
                if out.len() >= n {
                    break;
                }
                out.push(Line::from(span(fit(&format!("    {o}"), w), Role::Normal, t)));
            }
        }
    }
    if rows.is_empty() {
        out.push(Line::from(span("(nothing needs a decision)", Role::Queued, t)));
    }
    out
}

/// Detail block of one node (right column at 200 columns, top of the node panel).
fn node_detail_lines(nd: &NodeRow, w: usize, c: &Ctx) -> Vec<Line<'static>> {
    let t = c.theme;
    let rr = cov_role(nd);
    let l = |s: String, r: Role| Line::from(span(fit(&s, w), r, t));
    let mut v = vec![
        Line::from(vec![
            span("coverage ", Role::Label, t),
            span(cov_text(nd), rr, t),
            span(
                fit(
                    &format!(
                        "  target {}  spec box {}  phase {}  certified {}  attempt ended {}",
                        nd.target,
                        nd.spec_box.clone().unwrap_or_else(|| "-".into()),
                        nd.phase,
                        if nd.certified { "yes" } else { "no" },
                        if nd.attempt_ended { "yes" } else { "no" }
                    ),
                    w.saturating_sub(9 + cov_text(nd).len()),
                ),
                Role::Normal,
                t,
            ),
        ]),
        l(
            format!(
                "closure  rounds {}  defect {}  closure keys {}  rules used {} / {} ({})",
                cell(nd.closure_rounds),
                nd.defect_keys.map(thousands).unwrap_or_else(|| "not checked yet".into()),
                cell(nd.closure_keys),
                cell(nd.rules_used),
                cell(nd.rules_total),
                percent(nd.used_share)
            ),
            if nd.defect_keys.unwrap_or(0) > 0 { Role::Fail } else { Role::Normal },
        ),
        l(
            format!(
                "economy  rules {} -> {} (superseded {})  RHS terms {} -> {}",
                cell(nd.rules_before),
                cell(nd.rules_after),
                cell(nd.superseded),
                cell(nd.rhs_before),
                cell(nd.rhs_after)
            ),
            Role::Normal,
        ),
        l(
            format!(
                "terminals {} / {} (candidates {})  items open {}  CPU {} h",
                cell(nd.terminals),
                cell(nd.terminal_bound),
                cell(nd.terminal_candidates),
                cell(nd.items_open),
                cpu_hours(nd.cpu_seconds)
            ),
            Role::Terminal,
        ),
        l(
            format!(
                "evidence geometry {}  tested {}  proved {}  (cert {})",
                nd.accepted_by_evidence[0],
                nd.accepted_by_evidence[1],
                nd.accepted_by_evidence[2],
                percent(nd.certified_share)
            ),
            Role::Normal,
        ),
        l(
            format!(
                "blockers full: {}   box: {}",
                nd.blockers_full.clone().unwrap_or_else(|| "-".into()),
                nd.blockers_box.clone().unwrap_or_else(|| "-".into())
            ),
            if nd.blockers_full.is_some() || nd.blockers_box.is_some() { Role::Warn } else { Role::Normal },
        ),
    ];
    let phases: Vec<String> = PHASES
        .iter()
        .zip(nd.items_by_phase)
        .filter(|(_, n)| *n > 0)
        .map(|(p, n)| format!("{p} {n}"))
        .collect();
    v.push(l(
        format!(
            "items by state: {}",
            if phases.is_empty() { "-".into() } else { phases.join(", ") }
        ),
        Role::Normal,
    ));
    let by: Vec<String> = nd.open_by_rung.iter().map(|(r, k)| format!("{r} {k}")).collect();
    v.push(l(
        format!(
            "open by rung: {}",
            if by.is_empty() { "-".into() } else { by.join("  ") }
        ),
        Role::Normal,
    ));
    if let Some(s) = &nd.stale_reason {
        v.push(l(format!("STALE: {s}"), Role::Warn));
    }
    if let Some(p) = &nd.problem {
        v.push(l(format!("! {p}"), Role::Fail));
    }
    v
}

// ------------------------------------------------------------------------------------------------------------
// screens
// ------------------------------------------------------------------------------------------------------------

/// The plain status line inside a tiny terminal.
fn tiny(vm: &ViewModel, w: usize, h: usize, c: &Ctx) -> Vec<Line<'static>> {
    let line = super::plain::status_line(vm, c.app);
    let mut out: Vec<Line<'static>> = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    for chunk in chars.chunks(w.max(1)) {
        out.push(Line::from(span(chunk.iter().collect::<String>(), banner_role(vm.banner), c.theme)));
    }
    out.extend(banners(vm, w, c));
    out.push(Line::from(span(
        fit("enlarge to 80x24 for the dashboard", w),
        Role::Queued,
        c.theme,
    )));
    out.truncate(h.max(1));
    out
}

fn header_block(vm: &ViewModel, w: usize, c: &Ctx) -> Vec<Line<'static>> {
    let mut v = vec![header_line(vm, w, c)];
    v.extend(banners(vm, w, c));
    v.push(resources_line(vm, w, c));
    v.push(nodes_line(vm, w, c));
    v.push(items_line(vm, w, c));
    v.push(rules_line(vm, w, c));
    v.push(closure_line(vm, w, c));
    v.push(eta_line(vm, w, c));
    v
}

fn compact(vm: &ViewModel, w: usize, h: usize, c: &Ctx) -> Vec<Line<'static>> {
    let mut v = header_block(vm, w, c);
    let ev_rows = 3usize;
    let fixed = v.len() + 1 + 1 + ev_rows + 1;
    let avail = h.saturating_sub(fixed).max(2);
    let tab = c.app.compact;
    v.push(rule(tab.title(), "Tab: next table", w, c));
    let body = match tab {
        CompactTable::Ladder => ladder_table(vm, w, c),
        CompactTable::Nodes => nodes_table(vm, w, avail, WidthClass::Compact, c),
        CompactTable::Jobs => jobs_table(vm, w, avail, false, 0, c),
        CompactTable::Events => event_lines(&vm.events, w, avail, 0, c),
        CompactTable::Attention => attention_lines(&vm.attention, w, avail, true, c),
    };
    let mut body = body;
    body.truncate(avail);
    let spare = avail - body.len();
    v.extend(body);
    v.push(rule("Recent events", "e: all", w, c));
    let ev = if tab == CompactTable::Events { 0 } else { ev_rows + spare };
    if ev == 0 {
        v.extend(std::iter::repeat_n(Line::default(), ev_rows + spare));
    } else {
        v.extend(event_lines(&vm.events, w, ev, 0, c).into_iter().take(ev));
    }
    v.truncate(h.saturating_sub(1));
    while v.len() < h.saturating_sub(1) {
        v.push(Line::default());
    }
    v.push(footer(vm, w, WidthClass::Compact, c));
    v
}

fn default_body(vm: &ViewModel, w: usize, nodes_rows: usize, c: &Ctx, class: WidthClass) -> Vec<Line<'static>> {
    let mut v = vec![rule(
        &format!("Nodes ({})", vm.nodes.len()),
        "Enter: node detail",
        w,
        c,
    )];
    v.extend(nodes_table(vm, w, nodes_rows, class, c));
    let lw = (w / 2 + 6).clamp(60.min(w), 78);
    let rw = w.saturating_sub(lw + 2);
    let mut left = vec![rule("Work list by ladder level", "", lw, c)];
    left.extend(ladder_table(vm, lw, c));
    let mut right = vec![rule("Rules and certificates", "", rw, c)];
    right.extend(rules_panel(vm, rw, c));
    if rw >= 30 {
        v.extend(hjoin(left, lw, right, 2));
    } else {
        v.extend(left);
    }
    v
}

fn default_screen(vm: &ViewModel, w: usize, h: usize, c: &Ctx, class: WidthClass) -> Vec<Line<'static>> {
    let mut v = header_block(vm, w, c);
    let ladder_h = 1 + 1 + vm.ladder.len().max(1).max(7);
    // rows: nodes table gets up to 1/3 of the rest, jobs 1/4, events the remainder
    let rest = h.saturating_sub(v.len() + ladder_h + 1 + 3 + 1);
    let nodes_rows = (vm.nodes.len() + 1).clamp(2, (rest / 3).max(2));
    let jobs_rows = (jobs_rows(vm).map(|l| l.len()).unwrap_or(0) + 1).clamp(2, (rest / 4).max(2));
    let main = if class == WidthClass::Wide {
        let lw = 120usize.min(w);
        let rw = w.saturating_sub(lw + 2);
        let left = default_body(vm, lw, nodes_rows, c, WidthClass::Default);
        let mut right = Vec::new();
        if let Some(nd) = vm.nodes.get(c.app.selected_node.min(vm.nodes.len().saturating_sub(1))) {
            right.push(rule(&format!("{} (selected; n/N)", nd.id), "", rw, c));
            right.extend(node_detail_lines(nd, rw, c));
        }
        let room = left.len().saturating_sub(right.len() + 1).max(3);
        right.push(rule(&format!("Attention ({})", vm.attention.len()), "a: all", rw, c));
        right.extend(attention_lines(&vm.attention, rw, room, true, c));
        let n = left.len();
        let mut j = hjoin(left, lw, right, 2);
        j.truncate(n.max(j.len().min(n + 4)));
        j
    } else {
        default_body(vm, w, nodes_rows, c, class)
    };
    v.extend(main);
    let running = vm.jobs.running.map(|r| r.to_string()).unwrap_or_else(|| "?".into());
    v.push(rule(&format!("Running jobs {running}"), "j: all jobs", w, c));
    v.extend(jobs_table(vm, w, jobs_rows, class == WidthClass::Wide, 0, c));
    let ev_rows = h.saturating_sub(v.len() + 2).max(1);
    v.push(rule("Recent events", "e: all events", w, c));
    v.extend(event_lines(&vm.events, w, ev_rows, 0, c));
    v.truncate(h.saturating_sub(1));
    while v.len() < h.saturating_sub(1) {
        v.push(Line::default());
    }
    v.push(footer(vm, w, class, c));
    v
}

fn panel_screen(vm: &ViewModel, w: usize, h: usize, c: &Ctx) -> Vec<Line<'static>> {
    let t = c.theme;
    let mut v = vec![header_line(vm, w, c)];
    v.extend(banners(vm, w, c));
    let room = |v: &Vec<Line<'static>>| h.saturating_sub(v.len() + 2).max(1);
    match c.app.panel {
        Panel::Node => {
            let id = c.app.node_items.as_ref().map(|(id, _)| id.clone()).unwrap_or_default();
            v.push(rule(&format!("Node {id}"), "q: back", w, c));
            if let Some(nd) = vm.nodes.iter().find(|n| n.id == id) {
                v.extend(node_detail_lines(nd, w, c));
            }
            let items = c.app.visible_items();
            let filter = c.app.item_filter.map(|k| PHASES[k]).unwrap_or("all");
            let all = match &c.app.node_items {
                Some((_, Ok(v))) => v.len(),
                _ => 0,
            };
            v.push(rule(
                &format!("Items ({} of {all}; state: {filter})", items.len()),
                "1-9 filter  Enter/w why",
                w,
                c,
            ));
            match &c.app.node_items {
                Some((_, Err(e))) => v.push(Line::from(span(fit(&format!("! {e}"), w), Role::Warn, t))),
                _ => {
                    let cols = [
                        col("item", 18, Align::L, 0),
                        col("label", 22, Align::L, 0),
                        col("kind", 10, Align::L, 3),
                        col("rung", 4, Align::L, 0),
                        col("state", 18, Align::L, 0),
                        col("why", 40, Align::L, 1),
                        col("cpu s", 8, Align::R, 2),
                        col("jobs", 4, Align::R, 2),
                        col("seq", 8, Align::R, 4),
                    ];
                    let n = room(&v).saturating_sub(1).max(1);
                    let sel = c.app.selected_item.min(items.len().saturating_sub(1));
                    let (lo, hi) = window(items.len(), n, sel);
                    let rows: Vec<Cells> = items[lo..hi]
                        .iter()
                        .map(|i| {
                            let r = match i.state.as_str() {
                                "accepted" => Role::Ok,
                                "in_progress" => Role::Running,
                                "rejected" => Role::Fail,
                                "escalated" | "stale" | "terminal_candidate" => Role::Warn,
                                "terminal" => Role::Terminal,
                                "retired" => Role::Queued,
                                _ if phase_is_open(&i.state) => Role::Normal,
                                _ => Role::Normal,
                            };
                            vec![
                                (clean(&i.item), Role::Normal),
                                (clean(&i.label), Role::Normal),
                                (clean(&i.kind), Role::Queued),
                                (clean(&i.rung), Role::Label),
                                (clean(&i.state), r),
                                (clean(&i.why), Role::Normal),
                                (format!("{:.1}", i.cpu_seconds), Role::Normal),
                                (i.jobs.to_string(), Role::Normal),
                                (i.updated_seq.to_string(), Role::Queued),
                            ]
                        })
                        .collect();
                    v.extend(table(&cols, &rows, w, Some(sel - lo), c));
                }
            }
        }
        Panel::Item => {
            let (id, d) = match &c.app.item_detail {
                Some((id, d)) => (id.clone(), Some(d)),
                None => (String::new(), None),
            };
            v.push(rule(&format!("Item {id} (why)"), "q: back", w, c));
            match d {
                Some(Ok(val)) => v.extend(item_lines(val, w, c)),
                Some(Err(e)) => v.push(Line::from(span(fit(&format!("! {e}"), w), Role::Warn, t))),
                None => {}
            }
        }
        Panel::Jobs => {
            v.push(rule(&format!("Running jobs {}", count(vm.jobs.running)), "q: back", w, c));
            let n = room(&v);
            v.extend(jobs_table(vm, w, n, w >= 160, c.app.scroll, c));
        }
        Panel::Events => {
            v.push(rule(&format!("Recent events ({})", vm.events.len()), "q: back", w, c));
            let n = room(&v);
            let skip = c.app.scroll.min(vm.events.len().saturating_sub(1));
            v.extend(event_lines(&vm.events, w, n, skip, c));
        }
        Panel::Attention => {
            v.push(rule(&format!("Attention ({})", vm.attention.len()), "q: back", w, c));
            let n = room(&v);
            v.extend(attention_lines(&vm.attention, w, n, true, c));
        }
        Panel::Help => {
            v.push(rule("Help", "q: back", w, c));
            for s in help_text(vm) {
                v.push(Line::from(span(fit(&s, w), Role::Normal, t)));
            }
        }
        Panel::Summary => {}
    }
    v.truncate(h.saturating_sub(1));
    while v.len() < h.saturating_sub(1) {
        v.push(Line::default());
    }
    v.push(footer(vm, w, WidthClass::of(w as u16, h as u16), c));
    v
}

fn item_lines(d: &serde_json::Value, w: usize, c: &Ctx) -> Vec<Line<'static>> {
    let t = c.theme;
    let g = |p: &str| -> String {
        match d.pointer(p) {
            None | Some(serde_json::Value::Null) => "-".into(),
            Some(serde_json::Value::String(s)) => clean(s),
            Some(v) => clean(&v.to_string()),
        }
    };
    let mut v = vec![
        Line::from(vec![
            span("why ", Role::Label, t),
            span(fit(&g("/why"), w.saturating_sub(4)), Role::Warn, t),
        ]),
        Line::from(span(
            fit(
                &format!(
                    "node {}  label {}  kind {}  rung {}  phase {} {}",
                    g("/record/node"),
                    g("/record/label"),
                    g("/record/kind"),
                    g("/record/rung"),
                    g("/record/phase/phase"),
                    g("/record/phase/detail")
                ),
                w,
            ),
            Role::Normal,
            t,
        )),
        Line::from(span(
            fit(
                &format!(
                    "parent {}  weight {}  created seq {}  updated seq {}  cpu {} ms  terminal keys {}",
                    g("/record/parent"),
                    g("/record/weight"),
                    g("/record/created_seq"),
                    g("/record/updated_seq"),
                    g("/record/cpu_ms"),
                    g("/record/terminal_keys")
                ),
                w,
            ),
            Role::Normal,
            t,
        )),
        Line::from(span(fit(&format!("accepted {}", g("/record/accepted")), w), Role::Ok, t)),
        Line::from(span(fit(&format!("retired {}", g("/record/retired")), w), Role::Queued, t)),
        Line::from(span(fit(&format!("economy {}", g("/record/economy")), w), Role::Normal, t)),
        Line::from(span(
            fit(&format!("stale from {}  input {}", g("/record/stale_from"), g("/record/stale_input")), w),
            Role::Warn,
            t,
        )),
    ];
    v.push(Line::from(span("jobs:", Role::Label, t)));
    if let Some(a) = d.get("jobs").and_then(|j| j.as_array()) {
        for j in a.iter().take(50) {
            let g2 = |p: &str| -> String {
                match j.pointer(p) {
                    None | Some(serde_json::Value::Null) => "-".into(),
                    Some(serde_json::Value::String(s)) => clean(s),
                    Some(x) => clean(&x.to_string()),
                }
            };
            v.push(Line::from(span(
                fit(
                    &format!(
                        "  {} attempt {}  {} {}  cpu {} s  run dir {}",
                        g2("/job"),
                        g2("/attempt"),
                        g2("/state/state"),
                        g2("/state/class"),
                        g2("/cpu_seconds"),
                        g2("/run_dir")
                    ),
                    w,
                ),
                Role::Normal,
                t,
            )));
        }
    }
    v.push(Line::from(span(fit(&format!("children {}", g("/children")), w), Role::Normal, t)));
    v
}

fn help_text(vm: &ViewModel) -> Vec<String> {
    vec![
        "VIEW   Tab next table (80 cols)   n/N or arrows select   Enter/f/i node detail   w why (item)".into(),
        "       j running jobs   e events   a attention   ? help   q/Esc back   PgUp/PgDn/Home/End scroll".into(),
        "STOP   Ctrl-C once: drain (no new jobs; running ones finish or stop after the grace, recorded interrupted)".into(),
        "       Ctrl-C twice: forced stop, still safe   third: leave the dashboard at once   resume: the same command".into(),
        "       (in a read-only viewer Ctrl-C and q only close the viewer)".into(),
        "CONTROL  pause / resume / drain / retry / skip / terminal / workers: rr_campaign <verb> in another pane".into(),
        "COLOURS  green FULL/ok   blue [BOXED box] (closed inside the user's box)   magenta terminals   cyan RUN".into(),
        "         yellow escalated / attention / STALE / near a cap   red failed / rejected / OPEN with blockers / defect".into(),
        "         dim: estimates and unknown values (? and -: not known yet, never 0)".into(),
        "COVERAGE FULL = every key of the sector reduces; BOXED = closed inside the box GIVEN BY THE USER; OPEN = not closed".into(),
        "CERT%    accepted items at identity_proved / all accepted items".into(),
        "DEFECT   keys reached by the closure that are neither covered, nor terminals, nor handed down".into(),
        "USED%    rules used on the node's closure / rules of the node's dispatch (economy, R31)".into(),
        format!(
            "ETA      {}",
            vm.eta.estimator.clone().unwrap_or_else(|| "remaining items x per-rung CPU quantiles / parallelism".into())
        ),
        "STALE    the campaign says running but no heartbeat (campaign.json rewrite / resource sample) for > 60 s".into(),
        "DATA     progress/campaign.json, nodes/<node>.json, events.json; drill-down from a read-only ledger reduction".into(),
    ]
}

/// Builds every line of the frame.
pub fn screen_lines(vm: &ViewModel, w: u16, h: u16, c: &Ctx) -> Vec<Line<'static>> {
    let (wu, hu) = (w as usize, h as usize);
    if wu == 0 || hu == 0 {
        return Vec::new();
    }
    let class = WidthClass::of(w, h);
    if class == WidthClass::Tiny {
        return tiny(vm, wu, hu, c);
    }
    let mut v = if c.app.panel != Panel::Summary {
        panel_screen(vm, wu, hu, c)
    } else {
        match class {
            WidthClass::Compact => compact(vm, wu, hu, c),
            _ => default_screen(vm, wu, hu, c, class),
        }
    };
    v.truncate(hu);
    let _ = line_width;
    v
}

/// Draws the frame.
pub fn draw(frame: &mut Frame, vm: &ViewModel, c: &Ctx) {
    let area: Rect = frame.area();
    let lines = screen_lines(vm, area.width, area.height, c);
    frame.render_widget(Paragraph::new(lines), area);
}
