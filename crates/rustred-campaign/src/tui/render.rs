//! Screens. Everything is composed as lines of styled spans with columns whose widths come from the data (capped,
//! middle-cut for names), then drawn as one paragraph; nothing ever wraps. Tables are windowed: only visible rows are
//! built.
//!
//! Every span is made by [`span`] from a [`Clean`] (audit C1): the compiler refuses raw text here. [`gap`] makes
//! spaces. No other function of this file builds a `Span` (a test reads this file and checks it).
//!
//! The SUMMARY answers, top to bottom, the five-second questions: is it healthy (state, heartbeat, banners), how far
//! (PROGRESS), how long (ETA), what needs me (NEXT); then resources, certification and the detail lines and tables.
//!
//! Rendering pattern (TestBackend golden screens, a `screen()` text dump) follows rustred-app
//! `cli/shards/monitor.rs` (b9f06de7); layouts follow the plan's mock-ups of 3.U.2.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::app::{App, CompactTable, Panel, Slot};
use super::clean::{Clean, cl};
use super::format::{
    clock_range, count, countdown, cpu_h_range, cpu_hours, dur, gib, hms, percent, rough, thousands,
};
use super::model::{
    AttentionRow, Banner, Coverage, EventRow, EventTag, HeartbeatSource, JobView, JobsOrigin,
    LadderRow, NodeRow, PHASES, Severity, ViewModel,
};
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

type Cells = Vec<(Clean, Role)>;
type Lines = Vec<Line<'static>>;

/// THE span constructor: sanitised text only.
fn styled(s: Clean, st: Style) -> Span<'static> {
    Span::styled(s.into_string(), st)
}

/// A span in a role's style.
fn span(s: Clean, r: Role, t: Theme) -> Span<'static> {
    styled(s, t.style(r))
}

/// Spaces.
fn gap(n: usize) -> Span<'static> {
    Span::raw(" ".repeat(n))
}

fn text_line(s: impl AsRef<str>, w: usize, r: Role, t: Theme) -> Line<'static> {
    Line::from(span(cl(s).fit(w), r, t))
}

fn span_width(s: &Span) -> usize {
    s.width()
}

/// Pads (or cuts) a line to exactly `w` cells.
fn pad_line(mut l: Line<'static>, w: usize) -> Line<'static> {
    let mut used = 0usize;
    let mut out: Vec<Span<'static>> = Vec::new();
    for s in l.spans.drain(..) {
        let n = span_width(&s);
        if used + n <= w {
            used += n;
            out.push(s);
        } else {
            let keep = cl(s.content.as_ref()).fit(w - used);
            used += keep.width();
            out.push(styled(keep, s.style));
            break;
        }
    }
    if used < w {
        out.push(gap(w - used));
    }
    Line::from(out)
}

/// Joins two columns of lines side by side.
fn hjoin(left: Lines, lw: usize, right: Lines, g: usize) -> Lines {
    let n = left.len().max(right.len());
    let mut l = left.into_iter();
    let mut r = right.into_iter();
    (0..n)
        .map(|_| {
            let a = pad_line(l.next().unwrap_or_default(), lw);
            let mut spans = a.spans;
            spans.push(gap(g));
            if let Some(b) = r.next() {
                spans.extend(b.spans);
            }
            Line::from(spans)
        })
        .collect()
}

/// A section rule: `-[ title ]-----` (ASCII) or `─[ title ]─────` (UTF-8). The title may carry data (node ids).
fn rule(title: &str, hint: &str, w: usize, c: &Ctx) -> Line<'static> {
    let ch = if c.utf8 { '─' } else { '-' };
    let head = format!("{ch}[ {} ]", cl(title));
    let tail = if hint.is_empty() {
        String::new()
    } else {
        format!(" {hint} {ch}{ch}{ch}{ch}")
    };
    let fill = w.saturating_sub(cl(&head).width() + tail.chars().count());
    let mut s = head;
    s.extend(std::iter::repeat_n(ch, fill));
    s.push_str(&tail);
    Line::from(span(cl(s).fit(w), Role::Title, c.theme))
}

/// Adds segments (separated by `sep` spaces) while they fit into `w`; segments are in priority order.
fn segments(segs: Vec<Vec<Span<'static>>>, w: usize, sep: usize) -> Line<'static> {
    let mut out: Vec<Span<'static>> = Vec::new();
    let mut used = 0usize;
    for seg in segs {
        let n: usize = seg.iter().map(span_width).sum();
        if n == 0 {
            continue;
        }
        let extra = if out.is_empty() { 0 } else { sep };
        if used + extra + n > w {
            continue;
        }
        if extra > 0 {
            out.push(gap(extra));
        }
        used += extra + n;
        out.extend(seg);
    }
    Line::from(out)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Align {
    L,
    R,
}

/// A table column: title, width, alignment, drop priority (higher = dropped first when narrow; 99 = hidden), and
/// whether long values are cut in the middle (names) or at the end (lists).
struct Col {
    title: &'static str,
    width: usize,
    align: Align,
    drop: u8,
    mid: bool,
}

/// A column whose width is the widest of its title and cells, between `min` and `cap`.
fn col(
    title: &'static str,
    cells: impl Iterator<Item = usize>,
    min: usize,
    cap: usize,
    align: Align,
    drop: u8,
) -> Col {
    let w = cells
        .max()
        .unwrap_or(0)
        .max(title.chars().count())
        .clamp(min, cap.max(min));
    Col {
        title,
        width: w,
        align,
        drop,
        mid: false,
    }
}

impl Col {
    fn mid(mut self) -> Self {
        self.mid = true;
        self
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

fn cell_text(s: &Clean, c: &Col) -> Clean {
    let s = if c.mid {
        s.fit_mid(c.width)
    } else {
        s.fit(c.width)
    };
    s.pad(c.width, c.align == Align::R)
}

/// Renders a table: header + rows (already windowed by the caller); `selected` row is reversed.
fn table(cols: &[Col], rows: &[Cells], w: usize, selected: Option<usize>, c: &Ctx) -> Lines {
    let keep = fit_cols(cols, w);
    let mut out = Vec::with_capacity(rows.len() + 1);
    let mut head: Vec<Span<'static>> = Vec::new();
    for (k, &i) in keep.iter().enumerate() {
        if k > 0 {
            head.push(gap(1));
        }
        head.push(span(
            cell_text(&cl(cols[i].title), &cols[i]),
            Role::Label,
            c.theme,
        ));
    }
    out.push(Line::from(head));
    for (ri, row) in rows.iter().enumerate() {
        let sel = selected == Some(ri);
        let mut spans: Vec<Span<'static>> = Vec::new();
        for (k, &i) in keep.iter().enumerate() {
            if k > 0 {
                spans.push(gap(1));
            }
            let (text, role) = row.get(i).cloned().unwrap_or_default();
            let mut st = c.theme.style(role);
            if sel {
                st = st.patch(c.theme.style(Role::Selected));
            }
            spans.push(styled(cell_text(&text, &cols[i]), st));
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
// roles and small texts
// ------------------------------------------------------------------------------------------------------------

fn final_phase(n: &NodeRow) -> bool {
    matches!(n.phase.as_str(), "closed" | "certified" | "open")
}

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

fn cov_text(n: &NodeRow) -> Clean {
    let base = match n.coverage {
        Coverage::Boxed => format!("[BOXED {}]", n.target),
        Coverage::Full => "FULL".into(),
        Coverage::Open => "OPEN".into(),
        Coverage::Unknown => "?".into(),
    };
    cl(if n.provisional { base + " PROV" } else { base })
}

/// Certification asked / reached (`per-key/-`), or `yes` / `-` when the node file does not state levels.
fn cert_text(n: &NodeRow) -> Clean {
    cl(match (&n.cert_asked, &n.cert_reached) {
        (None, None) => if n.certified { "yes" } else { "-" }.to_string(),
        (a, r) => format!(
            "{}/{}",
            a.as_ref().map_or("?", Clean::as_str),
            r.as_ref().map_or("-", Clean::as_str)
        ),
    })
}

fn pref_text(n: &NodeRow) -> Clean {
    cl(match &n.preferred {
        None => "-".into(),
        Some(p) => format!(
            "{}/{}{}",
            cellv(p.used),
            cellv(p.total),
            p.not_needed.map(|k| format!(" nn{k}")).unwrap_or_default()
        ),
    })
}

fn ref_text(n: &NodeRow) -> Clean {
    cl(match &n.reference {
        None => "-".into(),
        Some(r) => {
            let mut v = Vec::new();
            if let Some(t) = r.terminals {
                v.push(format!("term {t}"));
            }
            if let Some(k) = r.rules {
                v.push(format!("rules {}", thousands(k)));
            }
            if v.is_empty()
                && let Some(st) = &r.status
            {
                v.push(st.to_string());
            }
            v.join(" ")
        }
    })
}

/// Table cell of an optional count: `-` when unknown or zero.
fn cellv(v: Option<u64>) -> String {
    match v {
        None => "-".into(),
        Some(n) => thousands(n),
    }
}

fn by_rung_text(n: &NodeRow) -> Clean {
    if n.open_by_rung.is_empty() {
        return cl("-");
    }
    cl(n.open_by_rung
        .iter()
        .map(|(r, k)| format!("{r}:{k}"))
        .collect::<Vec<_>>()
        .join(" "))
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
        Banner::Starting | Banner::Reading => Role::Running,
        Banner::Draining | Banner::Stale | Banner::Unknown => Role::Warn,
        Banner::Stopped => Role::Label,
        Banner::NoData | Banner::Unreadable => Role::Fail,
    }
}

/// Role of a used / cap pair: warn above 90 %.
fn cap_role(used: Option<u64>, cap: Option<u64>) -> Role {
    match (used, cap) {
        (Some(u), Some(c)) if c > 0 && u.saturating_mul(10) > c.saturating_mul(9) => Role::Warn,
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

/// Defect colour (audit m5): red only on a node whose attempt is over (closed / certified / open); while the node
/// is still running a defect is progress information (yellow).
fn defect_role(defect: Option<u64>, settled: bool) -> Role {
    match defect {
        Some(0) => Role::Ok,
        Some(_) if settled => Role::Fail,
        Some(_) => Role::Warn,
        None => Role::Estimate,
    }
}

fn kv(label: &str, value: impl AsRef<str>, vr: Role, t: Theme) -> Vec<Span<'static>> {
    vec![
        span(cl(format!("{label} ")), Role::Label, t),
        span(cl(value), vr, t),
    ]
}

/// A number whose source contradicts another: `?` appended, warning colour (audit M2).
fn marked(s: String, suspect: bool, r: Role) -> (String, Role) {
    if suspect {
        (s + "?", Role::Warn)
    } else {
        (s, r)
    }
}

/// A live-looking value (CPU, RAM, jobs, host memory): dimmed and suffixed with its age when old (audit M4); the
/// first value of a line carries the age, the others `(old)`.
fn live(vm: &ViewModel, s: String, r: Role, first: bool) -> (String, Role) {
    if vm.live_values_old() {
        let age = if first {
            format!("{} old", dur(vm.live_age_s()))
        } else {
            "old".to_string()
        };
        (format!("{s} ({age})"), Role::Estimate)
    } else {
        (s, r)
    }
}

// ------------------------------------------------------------------------------------------------------------
// header block
// ------------------------------------------------------------------------------------------------------------

/// The ETA sentence parts (a range, never one number): `(text, role)` in priority order.
pub fn eta_parts(vm: &ViewModel) -> Vec<(String, Role)> {
    let e = &vm.eta;
    let est = Role::Estimate;
    if vm.seq.is_none() {
        return vec![("ETA ?".into(), est)];
    }
    let range = match e.wall_s {
        Some((lo, hi)) => format!("{}-{}", rough(lo), rough(hi)),
        None => "?".into(),
    };
    if vm.live_values_old() {
        let why = if vm.banner == Banner::Stale {
            "supervisor not running"
        } else {
            "the progress files are slow to read"
        };
        return vec![(
            format!(
                "ETA ? ({why}; last estimate {range}, {} old)",
                dur(vm.live_age_s())
            ),
            Role::Warn,
        )];
    }
    if vm.banner == Banner::Finished {
        return vec![("ETA: finished".into(), Role::Ok)];
    }
    if vm.items_total == Some(0) {
        return vec![("ETA ? (no work registered yet)".into(), est)];
    }
    if e.remaining_items == Some(0) {
        return vec![("ETA: nothing left".into(), est)];
    }
    let basis = {
        let mut b = Vec::new();
        if let Some(n) = e.remaining_items {
            b.push(format!("{} items queued or running", thousands(n)));
        }
        if let Some(n) = e.parallelism {
            b.push(format!("{n} jobs"));
        }
        if !e.pooled_levels.is_empty() {
            b.push(format!(
                "pooled {}",
                e.pooled_levels
                    .iter()
                    .map(Clean::as_str)
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
        b.join(", ")
    };
    match (e.wall_s, e.cpu_s) {
        (Some((lo, hi)), Some((clo, chi))) => vec![
            (format!("ETA ~ {range}"), est),
            (
                format!(
                    "done ~ {}",
                    clock_range(vm.now_s, vm.now_s + lo, vm.now_s + hi)
                ),
                est,
            ),
            (
                format!("({} CPU-h left; {basis})", cpu_h_range(clo, chi)),
                est,
            ),
            ("escalations not counted".into(), est),
        ],
        (None, Some((clo, chi))) => vec![
            ("ETA ~ ? wall".into(), est),
            (format!("{} CPU-h left", cpu_h_range(clo, chi)), est),
            (
                format!(
                    "({})",
                    e.reason.as_ref().map_or("no parallelism", Clean::as_str)
                ),
                est,
            ),
        ],
        _ => vec![(
            format!(
                "ETA: too little data ({})",
                e.reason.as_ref().map_or("no estimate", Clean::as_str)
            ),
            est,
        )],
    }
}

/// The ETA as one sentence (plain mode, tests).
pub fn eta_text(vm: &ViewModel) -> String {
    eta_parts(vm)
        .into_iter()
        .map(|(s, _)| s)
        .collect::<Vec<_>>()
        .join("  ")
}

fn state_word(vm: &ViewModel, c: &Ctx) -> (String, Role) {
    if c.app.stop_count >= 2 {
        ("STOPPING (forced)".into(), Role::Warn)
    } else if c.app.stop_count == 1 {
        (
            format!(
                "STOPPING {}",
                countdown(c.app.grace_left(vm, c.now_s).unwrap_or(0.0))
            ),
            Role::Warn,
        )
    } else {
        (vm.banner.word().into(), banner_role(vm.banner))
    }
}

/// Header line 1: identity, state, heartbeat, ages.
fn header_line(vm: &ViewModel, w: usize, c: &Ctx) -> Line<'static> {
    let t = c.theme;
    let (sw, srole) = state_word(vm, c);
    let mut segs = vec![
        vec![span(cl("rr_campaign"), Role::Title, t)],
        vec![span(
            vm.campaign_id.clone().unwrap_or_else(|| cl("?")),
            Role::Label,
            t,
        )],
        vec![span(cl(sw), srole, t)],
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
        kv(vm.elapsed_label, dur(vm.elapsed_s), Role::Normal, t),
        kv("seq", count(vm.seq), Role::Normal, t),
    ];
    if let Some(a) = vm.data_age_s.filter(|a| *a > 5.0) {
        segs.insert(
            3,
            kv("data", format!("{} old", dur(Some(a))), Role::Warn, t),
        );
    }
    if let Some(s) = vm.sessions {
        segs.push(kv("sessions", s.to_string(), Role::Normal, t));
    }
    let rec = vm.crash_recoveries.unwrap_or(0)
        + vm.lock_takeovers.unwrap_or(0)
        + vm.torn_tail_recoveries.unwrap_or(0);
    if rec > 0 {
        segs.push(kv("recoveries", rec.to_string(), Role::Warn, t));
    }
    if let Some(m) = &vm.mode {
        segs.push(kv("mode", m, Role::Label, t));
    }
    if let Some(a) = &vm.cert_asked {
        segs.push(kv("cert asked", a, Role::Normal, t));
    }
    if let Some(ti) = &vm.title {
        segs.push(vec![span(ti.clone(), Role::Normal, t)]);
    }
    segs.push(kv("dir", &vm.origin, Role::Queued, t));
    segments(segs, w, 2)
}

fn bar(done: u64, total: u64, cells: usize, c: &Ctx) -> String {
    let (f, e) = if c.utf8 { ('█', '░') } else { ('#', '.') };
    let k = if total == 0 {
        0
    } else {
        ((done as f64 / total as f64) * cells as f64).round() as usize
    }
    .min(cells);
    let mut s: String = std::iter::repeat_n(f, k).collect();
    s.extend(std::iter::repeat_n(e, cells - k));
    s
}

/// PROGRESS: nodes closed, items done (bar), last event.
fn progress_line(vm: &ViewModel, w: usize, c: &Ctx) -> Line<'static> {
    let t = c.theme;
    let ns = vm.suspect.contains("nodes");
    let (tot, tr) = marked(count(vm.nodes_total), ns, Role::Normal);
    let mut segs = vec![
        kv("NODES", tot, tr, t),
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
    ];
    if let (Some(done), Some(total)) = (vm.items_done(), vm.items_total)
        && total > 0
    {
        let cells = if w >= 120 { 20 } else { 10 };
        let (txt, r) = marked(
            format!(
                "{:.0}% ({} of {})",
                done as f64 * 100.0 / total as f64,
                thousands(done),
                thousands(total)
            ),
            vm.suspect.contains("items"),
            Role::Normal,
        );
        segs.push(vec![
            span(cl("items done "), Role::Label, t),
            span(cl(bar(done, total, cells, c)), Role::Ok, t),
            gap(1),
            span(cl(txt), r, t),
        ]);
    }
    if vm.faces_total.is_some() {
        segs.push(kv(
            "faces",
            format!("{}/{}", count(vm.faces_closed), count(vm.faces_total)),
            Role::Normal,
            t,
        ));
    }
    if let Some(a) = vm.last_event_age_s {
        segs.push(kv(
            "last event",
            format!("{} ago", dur(Some(a))),
            Role::Queued,
            t,
        ));
    }
    segments(segs, w, 2)
}

fn eta_line(vm: &ViewModel, w: usize, c: &Ctx) -> Line<'static> {
    let t = c.theme;
    let mut segs: Vec<Vec<Span<'static>>> = Vec::new();
    for (i, (s, r)) in eta_parts(vm).into_iter().enumerate() {
        // the first part (the range) is never dropped: cut to the width
        let s = if i == 0 { cl(s).fit(w) } else { cl(s) };
        segs.push(vec![span(s, r, t)]);
    }
    // the cost so far right after the range and the finishing time (it is a website = terminal number)
    let at = segs.len().min(2);
    segs.insert(
        at,
        kv(
            "cpu-h",
            format!(
                "{}{} used",
                cpu_hours(vm.cost_cpu_s),
                vm.budget_cpu_hours
                    .map(|b| format!("/{b:.0}"))
                    .unwrap_or_default()
            ),
            Role::Normal,
            t,
        ),
    );
    segments(segs, w, 2)
}

/// NEXT: the most important thing that needs the user, with the command to type.
fn next_line(vm: &ViewModel, w: usize, c: &Ctx) -> Line<'static> {
    let t = c.theme;
    let head = span(cl("NEXT  "), Role::Label, t);
    match vm.attention.first() {
        None => Line::from(vec![head, span(cl("nothing needs you"), Role::Ok, t)]),
        Some(a) => {
            let role = match a.severity {
                Severity::Error => Role::Fail,
                _ => Role::Warn,
            };
            let n_more = vm.attention.len() - 1;
            let cmd = a
                .options
                .first()
                .map(|o| format!(" -> {o}"))
                .unwrap_or_default();
            // the full sentence when it fits, else the code and the command to type (the command matters most)
            let mut more = cl(if n_more > 0 {
                format!("  (+{n_more} more: a)")
            } else {
                String::new()
            });
            let mut body = cl(format!("{}: {}{cmd}", a.id, a.text));
            if 6 + body.width() + more.width() > w {
                body = cl(format!("{}: {}{cmd}", a.id, a.code));
                if n_more > 0 {
                    more = cl(format!("  (+{n_more}: a)"));
                }
            }
            let room = w.saturating_sub(6 + more.width());
            Line::from(vec![
                head,
                span(body.fit(room), role, t),
                span(more, Role::Queued, t),
            ])
        }
    }
}

fn resources_line(vm: &ViewModel, w: usize, c: &Ctx) -> Line<'static> {
    let t = c.theme;
    let r = &vm.resources;
    let (cpu, cpur) = live(
        vm,
        format!("{}/{} in use", count(r.cpus_in_use), count(r.cpus_allowed)),
        cap_role(r.cpus_in_use, r.cpus_allowed),
        true,
    );
    // used / cap, then the reservation (all GiB, written once)
    let ram_text = format!(
        "{}/{} GiB ({} reserved)",
        gib(r.rss_bytes),
        gib(r.mem_cap_bytes),
        gib(r.reserved_bytes)
    );
    let (ram, ramr) = live(
        vm,
        ram_text,
        cap_role(r.rss_bytes.max(r.reserved_bytes), r.mem_cap_bytes),
        false,
    );
    let mut jobs = format!("{} running", count(vm.jobs.running));
    if let Some(cap) = vm.jobs.cap {
        jobs.push_str(&format!(" of {cap}"));
    }
    if let Some(q) = vm.jobs.queued.filter(|q| *q > 0) {
        jobs.push_str(&format!(", {q} queued"));
    }
    if let Some(l) = vm.jobs.launching.filter(|n| *n > 0) {
        jobs.push_str(&format!(", {l} launching"));
    }
    let (jobs, jr) = live(vm, jobs, Role::Running, false);
    let (host, hr) = live(
        vm,
        format!(
            "{} GiB (floor {})",
            gib(r.host_available_bytes),
            gib(r.host_floor_bytes)
        ),
        host_role(r.host_available_bytes, r.host_floor_bytes),
        false,
    );
    let mut segs = vec![
        kv("CPU", cpu, cpur, t),
        kv("jobs", jobs, jr, t),
        kv("RAM", ram, ramr, t),
        kv("host free", host, hr, t),
    ];
    if let Some(d) = r.disk_free_bytes {
        segs.push(kv(
            "disk free",
            super::format::bytes(Some(d)),
            Role::Normal,
            t,
        ));
    }
    segments(segs, w, 3)
}

/// CERTIFIED at the level each node's spec asks (audit M3), rules by level, DEFECT.
fn cert_line(vm: &ViewModel, w: usize, c: &Ctx) -> Line<'static> {
    let t = c.theme;
    let cv = &vm.cert;
    let all = cv.nodes > 0 && cv.nodes_at_level == cv.nodes;
    let (nodes_txt, nr) = marked(
        format!(
            "{} of {} nodes{}",
            cv.nodes_at_level,
            cv.nodes,
            if cv.levels_unstated && cv.nodes > 0 {
                " (levels not stated: certified flag)"
            } else {
                ""
            }
        ),
        vm.suspect.contains("cert"),
        if all { Role::Ok } else { Role::Normal },
    );
    // levels by name, the per-key level (proofs of single rules) last, then the pending rules
    let mut levels: Vec<(&Clean, &u64)> = cv.rules_by_level.iter().collect();
    levels.sort_by_key(|(l, _)| (*l == super::model::PER_KEY, (*l).clone()));
    let mut rules: Vec<String> = levels
        .into_iter()
        .map(|(l, n)| {
            if l.as_str().contains(' ') || l == super::model::PER_KEY {
                format!("{} {l}", thousands(*n))
            } else {
                format!("{} at {l}", thousands(*n))
            }
        })
        .collect();
    rules.push(format!("{} pending", thousands(cv.rules_pending)));
    let head = kv("CERTIFIED at the level asked:", nodes_txt, nr, t);
    let used: usize = head.iter().map(span_width).sum();
    let mut spans = head;
    if used + 4 < w {
        spans.push(span(
            cl(format!("; rules: {}", rules.join(", "))).fit(w - used),
            Role::Normal,
            t,
        ));
    }
    Line::from(spans)
}

fn defect_seg(vm: &ViewModel, t: Theme) -> Vec<Span<'static>> {
    let settled = vm
        .nodes
        .iter()
        .any(|n| n.defect_keys.unwrap_or(0) > 0 && final_phase(n));
    let (defect, dr) = marked(
        match vm.defect_keys {
            None => "not checked yet".to_string(),
            Some(d) => format!(
                "{} keys ({}/{} checked nodes)",
                thousands(d),
                count(vm.nodes_with_defect),
                count(vm.nodes_checked)
            ),
        },
        vm.suspect.contains("defect"),
        defect_role(vm.defect_keys, settled),
    );
    kv("DEFECT", defect, dr, t)
}

fn items_line(vm: &ViewModel, w: usize, c: &Ctx) -> Line<'static> {
    let t = c.theme;
    let ph = |p: &str| vm.items_by_phase[PHASES.iter().position(|x| *x == p).unwrap_or(0)];
    let (tot, tr) = marked(
        count(vm.items_total),
        vm.suspect.contains("items"),
        Role::Normal,
    );
    let segs = vec![
        kv("ITEMS", tot, tr, t),
        kv("open", count(vm.items_open), Role::Normal, t),
        kv("run", count(ph("in_progress")), Role::Running, t),
        kv("acc", count(ph("accepted")), Role::Ok, t),
        kv("esc", count(ph("escalated")), Role::Warn, t),
        kv("rej", count(ph("rejected")), Role::Fail, t),
        kv("term", count(ph("terminal")), Role::Terminal, t),
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
        defect_seg(vm, t),
        kv(
            "TERMINALS",
            format!(
                "{} (cand {}, bound {}/node{})",
                count(vm.terminals_declared),
                count(vm.terminal_candidates),
                count(vm.terminal_bound),
                vm.preferred
                    .as_ref()
                    .map(|p| format!(", pref {}/{}", cellv(p.used), cellv(p.total)))
                    .unwrap_or_default()
            ),
            Role::Terminal,
            t,
        ),
        kv(
            "ECONOMY rules",
            format!(
                "{}->{}, RHS {}->{}",
                count(r.rules_before),
                count(r.rules_after),
                count(r.rhs_before),
                count(r.rhs_after)
            ),
            Role::Normal,
            t,
        ),
    ];
    segments(segs, w, 2)
}

/// Banner lines: stop protocol, READING / data lag, STALE, problems, transient message.
fn banners(vm: &ViewModel, w: usize, c: &Ctx) -> Lines {
    let t = c.theme;
    let mut out = Vec::new();
    let running = vm.jobs.running.unwrap_or(0) + vm.jobs.launching.unwrap_or(0);
    if c.app.stop_count >= 2 {
        out.push(text_line(
            format!(
                " FORCED STOP: {running} running job(s) are being killed and recorded as interrupted; still safe (only sealed outputs are accepted). Resume with the same command. "
            ),
            w,
            Role::BannerFail,
            t,
        ));
    } else if c.app.stop_count == 1 {
        let left = c.app.grace_left(vm, c.now_s).unwrap_or(0.0);
        out.push(text_line(
            format!(
                " STOPPING {}  draining: {running} job(s) running, grace {} s; nothing half-accepted. Press Ctrl-C again to force. ",
                countdown(left),
                c.app.grace_s
            ),
            w,
            Role::BannerWarn,
            t,
        ));
    }
    if vm.banner == Banner::Stale {
        let since = vm
            .heartbeat_utc
            .as_ref()
            .map(|h| format!("since {} UTC ", hms(h.as_str())))
            .unwrap_or_default();
        let (age, limit) = (dur(vm.heartbeat_age_s), vm.stale_after_s as u64);
        let what = match vm.heartbeat_source {
            HeartbeatSource::Field => format!("(no heartbeat for {age}, > {limit} s)"),
            _ => format!(
                "({age}, > {limit} s) [no heartbeat field: staleness inferred from file times]"
            ),
        };
        out.push(text_line(
            format!(" STALE {since}{what}: supervisor not running? Resume with the same command. "),
            w,
            Role::BannerWarn,
            t,
        ));
    } else if vm.banner != Banner::Reading
        && let Some(a) = vm.data_age_s.filter(|a| *a > 10.0)
    {
        out.push(text_line(
            format!(
                " DATA {} OLD: the progress files are slow to read; the values shown are from the last read ",
                dur(Some(a))
            ),
            w,
            Role::BannerWarn,
            t,
        ));
    }
    if matches!(
        vm.banner,
        Banner::NoData | Banner::Unreadable | Banner::Reading
    ) || !vm.problems.is_empty()
    {
        let more = if vm.problems.len() > 1 {
            format!(" (+{} more: ? help lists them)", vm.problems.len() - 1)
        } else {
            String::new()
        };
        let first = vm.problems.first().cloned().unwrap_or_default();
        let role = if vm.banner == Banner::Reading {
            Role::Running
        } else {
            Role::Fail
        };
        out.push(text_line(format!("! {first}{more}"), w, role, t));
    }
    if let Some(m) = c.app.message_at(c.now_s) {
        out.push(text_line(format!("> {m}"), w, Role::Warn, t));
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
        (Panel::Node, _) => {
            format!(
                "n/N item  Enter/w why  1-9,0 filter by state (* all)  PgUp/PgDn  q back  {stop}"
            )
        }
        _ => format!("arrows/PgUp/PgDn scroll  q back  {stop}"),
    };
    text_line(s, w, Role::Queued, c.theme)
}

// ------------------------------------------------------------------------------------------------------------
// tables
// ------------------------------------------------------------------------------------------------------------

/// Node id with the stale marker.
fn node_label(nd: &NodeRow) -> Clean {
    if nd.stale_reason.is_some() || nd.attention {
        cl(format!("!{}", nd.id))
    } else {
        nd.id.clone()
    }
}

fn nodes_table(vm: &ViewModel, w: usize, rows: usize, class: WidthClass, c: &Ctx) -> Lines {
    let compact = class == WidthClass::Compact;
    let ns = &vm.nodes;
    let has_faces = ns.iter().any(|n| n.faces_total.is_some());
    let has_lvl = ns
        .iter()
        .any(|n| n.cert_asked.is_some() || n.cert_reached.is_some());
    let has_pref = ns.iter().any(|n| n.preferred.is_some());
    let has_ref = ns.iter().any(|n| n.reference.is_some());
    let optimise = vm.mode.as_ref().is_some_and(|m| m == "optimise");
    let total = ns.len();
    let sel = c.app.selected_node.min(total.saturating_sub(1));
    let n = rows.saturating_sub(1).max(1);
    let (lo, hi) = window(
        total,
        if total > n {
            n.saturating_sub(1).max(1)
        } else {
            n
        },
        sel,
    );
    let data: Vec<Cells> = ns[lo..hi]
        .iter()
        .map(|nd| {
            let rr = cov_role(nd);
            let row_role = if nd.coverage == Coverage::Boxed {
                Role::Boxed
            } else {
                Role::Normal
            };
            let term = cl(match (nd.terminals, nd.terminal_bound) {
                (Some(t), Some(b)) => format!("{t}/{b}"),
                (Some(t), None) => t.to_string(),
                _ => "-".into(),
            });
            let (rules, rr2) = marked(
                if optimise {
                    format!("{}>{}", cellv(nd.rules_before), cellv(nd.rules_after))
                } else {
                    cellv(nd.rules_in_use)
                },
                nd.suspect.contains("rules"),
                row_role,
            );
            let (used, ur) = marked(
                percent(nd.used_share),
                nd.suspect.contains("used"),
                row_role,
            );
            let (open, or) = marked(cellv(nd.items_open), nd.suspect.contains("open"), row_role);
            vec![
                (node_label(nd), row_role),
                (cov_text(nd), rr),
                (
                    cert_text(nd),
                    if nd.at_level() { Role::Ok } else { row_role },
                ),
                (
                    if nd.problem.is_some() {
                        cl("unreadable")
                    } else {
                        nd.phase.clone()
                    },
                    if nd.problem.is_some() {
                        Role::Fail
                    } else {
                        row_role
                    },
                ),
                (
                    cl(match (nd.faces_closed, nd.faces_total) {
                        (Some(a), Some(b)) => format!("{a}/{b}"),
                        _ => "-".into(),
                    }),
                    row_role,
                ),
                (cl(open), or),
                (by_rung_text(nd), row_role),
                (term, Role::Terminal),
                (pref_text(nd), Role::Terminal),
                (
                    cl(cellv(nd.defect_keys)),
                    defect_role(nd.defect_keys, final_phase(nd)),
                ),
                (cl(rules), rr2),
                (cl(used), ur),
                (
                    cl(if optimise {
                        format!("{}>{}", cellv(nd.rhs_before), cellv(nd.rhs_after))
                    } else {
                        cellv(nd.rhs_after)
                    }),
                    row_role,
                ),
                (cl(percent(nd.proved_share)), row_role),
                (cl(cpu_hours(nd.cpu_seconds)), row_role),
                // a reference is information: neutral colour, never an error colour
                (ref_text(nd), Role::Queued),
            ]
        })
        .collect();
    let wi = |i: usize| data.iter().map(move |r| r[i].0.width());
    let cols = [
        col("node", wi(0), 4, 24, Align::L, 0).mid(),
        col("coverage", wi(1), 4, 28, Align::L, 0).mid(),
        col(
            if has_lvl { "cert asked/got" } else { "cert" },
            wi(2),
            3,
            24,
            Align::L,
            if has_lvl { 3 } else { 8 },
        ),
        col("phase", wi(3), 5, 14, Align::L, 3),
        col(
            "faces",
            wi(4),
            5,
            12,
            Align::R,
            if !has_faces {
                99
            } else if compact {
                9
            } else {
                5
            },
        ),
        col("open", wi(5), 4, 9, Align::R, 0),
        col(
            "open by rung",
            wi(6),
            6,
            40,
            Align::L,
            if compact { 9 } else { 5 },
        ),
        col("term", wi(7), 4, 11, Align::R, 1),
        col(
            "pref",
            wi(8),
            4,
            14,
            Align::R,
            if has_pref { 4 } else { 99 },
        ),
        col("defect", wi(9), 6, 12, Align::R, 1),
        col(
            if optimise { "rules bef>aft" } else { "rules" },
            wi(10),
            5,
            21,
            Align::R,
            2,
        ),
        col("used%", wi(11), 6, 7, Align::R, 2),
        col(
            if optimise { "RHS bef>aft" } else { "RHS" },
            wi(12),
            5,
            25,
            Align::R,
            if compact { 9 } else { 6 },
        ),
        col(
            "proved%",
            wi(13),
            7,
            7,
            Align::R,
            if compact { 9 } else { 7 },
        ),
        col("CPU-h", wi(14), 5, 9, Align::R, if compact { 9 } else { 6 }),
        col(
            "reference (info)",
            wi(15),
            8,
            30,
            Align::L,
            if has_ref { 7 } else { 99 },
        ),
    ];
    let mut out = table(&cols, &data, w, Some(sel - lo), c);
    if hi - lo < total {
        out.push(text_line(
            format!(
                "... {} more (n/N to scroll; {} of {total})",
                total - (hi - lo),
                sel + 1
            ),
            w,
            Role::Queued,
            c.theme,
        ));
    }
    if total == 0 {
        out.push(text_line("(no node files yet)", w, Role::Queued, c.theme));
    }
    out
}

/// The work list by ladder level: rung names and counts from the data; a phase column appears only when some rung
/// has items in that phase (fewer columns of `-`).
fn ladder_table(vm: &ViewModel, w: usize, c: &Ctx) -> Lines {
    let role_of = |p: &str| match p {
        "in_progress" => Role::Running,
        "accepted" => Role::Ok,
        "rejected" => Role::Fail,
        "escalated" | "stale" | "terminal_candidate" => Role::Warn,
        "terminal" => Role::Terminal,
        "retired" => Role::Queued,
        _ => Role::Normal,
    };
    let phases: [(&str, &'static str, u8); 9] = [
        ("in_progress", "run", 0),
        ("produced", "prod", 3),
        ("accepted", "acc", 0),
        ("rejected", "rej", 1),
        ("escalated", "esc", 1),
        ("terminal_candidate", "tcand", 4),
        ("terminal", "term", 1),
        ("stale", "stale", 4),
        ("retired", "ret", 4),
    ];
    let idx = |p: &str| PHASES.iter().position(|x| *x == p).unwrap_or(0);
    let shown: Vec<bool> = phases
        .iter()
        .map(|(p, _, d)| *d == 0 || vm.ladder.iter().any(|l| l.counts[idx(p)] > 0))
        .collect();
    let rows: Vec<Cells> = vm
        .ladder
        .iter()
        .map(|l: &LadderRow| {
            let mut v: Cells = vec![(l.rung.clone(), Role::Label)];
            let open = l.open();
            v.push((cl(cellv(Some(open).filter(|o| *o > 0))), Role::Normal));
            for (p, _, _) in phases {
                let k = l.counts[idx(p)];
                v.push((
                    cl(if k == 0 { "-".into() } else { thousands(k) }),
                    if k == 0 { Role::Queued } else { role_of(p) },
                ));
            }
            v.push((cl(cellv(l.jobs)), Role::Normal));
            v.push((
                cl(l.median_s
                    .map(|m| format!("{m:.1}"))
                    .unwrap_or_else(|| "-".into())),
                Role::Estimate,
            ));
            v
        })
        .collect();
    let wi = |i: usize| rows.iter().map(move |r| r[i].0.width());
    let mut cols = vec![
        col("rung", wi(0), 4, 16, Align::L, 0).mid(),
        col("open", wi(1), 4, 9, Align::R, 0),
    ];
    for (k, (_, title, d)) in phases.iter().enumerate() {
        cols.push(col(
            title,
            wi(2 + k),
            3,
            9,
            Align::R,
            if shown[k] { *d } else { 99 },
        ));
    }
    cols.push(col("jobs", wi(11), 4, 9, Align::R, 2));
    cols.push(col("med s", wi(12), 5, 9, Align::R, 2));
    let mut out = table(&cols, &rows, w, None, c);
    if rows.is_empty() {
        out.push(text_line("(no items yet)", w, Role::Queued, c.theme));
    }
    out
}

/// Natural width of the ladder table (all shown columns).
fn ladder_width(vm: &ViewModel, c: &Ctx) -> usize {
    ladder_table(vm, 400, c)
        .iter()
        .map(|l| l.width())
        .max()
        .unwrap_or(0)
}

/// Counts as `name n` sorted by count (largest first), so a cut at the panel edge hides the small ones.
fn by_count(m: &std::collections::BTreeMap<Clean, u64>) -> String {
    let mut v: Vec<(&Clean, &u64)> = m.iter().filter(|(_, n)| **n > 0).collect();
    v.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
    if v.is_empty() {
        return "-".into();
    }
    v.iter()
        .map(|(k, n)| format!("{k} {}", thousands(**n)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn rules_panel(vm: &ViewModel, w: usize, c: &Ctx) -> Lines {
    let t = c.theme;
    let r = &vm.rules;
    let cv = &vm.cert;
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
    out.push(text_line(
        format!("rejected: {}", by_count(&r.rejected_by_reason)),
        w,
        Role::Normal,
        t,
    ));
    out.push(text_line(
        format!(
            "evidence: geometry {}  tested {}  proved {}",
            count(r.by_evidence[0]),
            count(r.by_evidence[1]),
            count(r.by_evidence[2])
        ),
        w,
        Role::Normal,
        t,
    ));
    out.push(text_line(
        format!(
            "per-key proved {} of {} accepted ({})",
            count(cv.per_key_proved),
            count(cv.per_key_accepted),
            percent(cv.per_key_share())
        ),
        w,
        Role::Normal,
        t,
    ));
    out.push(text_line(
        format!(
            "economy: rules {}->{}  RHS {}->{}",
            count(r.rules_before),
            count(r.rules_after),
            count(r.rhs_before),
            count(r.rhs_after)
        ),
        w,
        Role::Normal,
        t,
    ));
    let worst = vm
        .nodes
        .iter()
        .filter(|n| n.defect_keys.unwrap_or(0) > 0)
        .max_by_key(|n| n.defect_keys.unwrap_or(0));
    out.push(text_line(
        match (vm.defect_keys, worst) {
            (None, _) => "closure defect: not checked yet".to_string(),
            (Some(d), Some(n)) => format!(
                "closure defect {} keys (largest {} {})",
                thousands(d),
                n.id,
                thousands(n.defect_keys.unwrap_or(0))
            ),
            (Some(d), None) => format!("closure defect {} keys", thousands(d)),
        },
        w,
        defect_role(
            vm.defect_keys,
            vm.nodes
                .iter()
                .any(|n| n.defect_keys.unwrap_or(0) > 0 && final_phase(n)),
        ),
        t,
    ));
    out.push(text_line(
        format!(
            "jobs {} settled: {}",
            count(vm.jobs.total),
            by_count(&vm.jobs.settled_by_class)
        ),
        w,
        Role::Normal,
        t,
    ));
    out
}

fn jobs_title(vm: &ViewModel) -> String {
    let src = match vm.jobs.list_origin {
        JobsOrigin::LedgerPartial => " (from ledger, partial: no CPU / RSS / elapsed)",
        _ => "",
    };
    format!("Running jobs {}{src}", count(vm.jobs.running))
}

fn jobs_table(vm: &ViewModel, w: usize, rows: usize, wide: bool, scroll: usize, c: &Ctx) -> Lines {
    let Some(list) = vm.jobs.list.as_ref() else {
        return vec![text_line(
            format!(
                "running {} (no job list in the progress files; ledger fallback not available)",
                count(vm.jobs.running)
            ),
            w,
            Role::Queued,
            c.theme,
        )];
    };
    let total = list.len();
    let n = rows.saturating_sub(1).max(1);
    let lo = scroll.min(total.saturating_sub(1));
    let hi = (lo
        + if total - lo > n {
            n.saturating_sub(1).max(1)
        } else {
            n
        })
    .min(total);
    let data: Vec<Cells> = list[lo..hi]
        .iter()
        .map(|j: &JobView| {
            let rss = match (j.rss_bytes, j.cap_bytes) {
                (None, None) => "-".into(),
                (a, b) => format!(
                    "{} / {}",
                    if a.is_some() { gib(a) } else { "-".into() },
                    if b.is_some() { gib(b) } else { "-".into() }
                ),
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
                (cl(cellv(j.cpus)), Role::Normal),
                (cl(rss), cap_role(j.rss_bytes, j.cap_bytes)),
                (cl(el), Role::Normal),
                (
                    cl(j.heartbeat_age_s
                        .map(|a| dur(Some(a)))
                        .unwrap_or_else(|| "-".into())),
                    Role::Normal,
                ),
                (j.phase.clone().unwrap_or_else(|| cl("-")), Role::Queued),
                (
                    cl(j.pid.map(|p| p.to_string()).unwrap_or_else(|| "-".into())),
                    Role::Queued,
                ),
                (j.run_dir.clone().unwrap_or_else(|| cl("-")), Role::Queued),
            ]
        })
        .collect();
    let wi = |i: usize| data.iter().map(move |r| r[i].0.width());
    let cols = [
        col("job", wi(0), 3, 20, Align::L, 0).mid(),
        col("node", wi(1), 4, 24, Align::L, 0).mid(),
        col("item", wi(2), 4, 30, Align::L, 1).mid(),
        col("rung", wi(3), 4, 12, Align::L, 0),
        col("cpu", wi(4), 3, 5, Align::R, 3),
        col("GiB RSS / cap", wi(5), 9, 17, Align::R, 2),
        col("elapsed / budget", wi(6), 9, 19, Align::R, 2),
        col("hb", wi(7), 3, 7, Align::R, 4),
        col("phase", wi(8), 5, 14, Align::L, 3),
        col("pid", wi(9), 3, 10, Align::R, if wide { 5 } else { 99 }),
        col(
            "run dir",
            wi(10),
            7,
            48,
            Align::L,
            if wide { 5 } else { 99 },
        )
        .mid(),
    ];
    let mut out = table(&cols, &data, w, None, c);
    if total == 0 {
        out.push(text_line("(no job running)", w, Role::Queued, c.theme));
    } else if hi - lo < total {
        out.push(text_line(
            format!("... {} more (j: all jobs)", total - (hi - lo)),
            w,
            Role::Queued,
            c.theme,
        ));
    }
    out
}

fn event_lines(events: &[EventRow], w: usize, n: usize, skip: usize, kind: bool, c: &Ctx) -> Lines {
    let t = c.theme;
    let mut out: Lines = events
        .iter()
        .skip(skip)
        .take(n)
        .map(|e| {
            let head = 9 + 5;
            let rest = if kind {
                format!("{} {}", e.kind.pad(17, false), e.text)
            } else {
                e.text.to_string()
            };
            Line::from(vec![
                span(cl(hms(e.utc.as_str())), Role::Queued, t),
                gap(1),
                span(cl(e.tag.word()).pad(4, false), tag_role(e.tag), t),
                gap(1),
                span(cl(rest).fit(w.saturating_sub(head)), Role::Normal, t),
            ])
        })
        .collect();
    if events.is_empty() {
        out.push(text_line("(no events yet)", w, Role::Queued, t));
    }
    out
}

fn attention_lines(
    rows: &[AttentionRow],
    w: usize,
    n: usize,
    with_options: bool,
    c: &Ctx,
) -> Lines {
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
            span(cl("! "), role, t),
            span(
                cl(format!(
                    "{} {} [{}] {}{}",
                    a.id,
                    a.code,
                    a.severity.word(),
                    a.text,
                    if a.derived { " (derived)" } else { "" }
                ))
                .fit(w.saturating_sub(2)),
                role,
                t,
            ),
        ]));
        if with_options {
            for o in &a.options {
                if out.len() >= n {
                    break;
                }
                out.push(text_line(format!("    {o}"), w, Role::Normal, t));
            }
        }
    }
    if rows.is_empty() {
        out.push(text_line("(nothing needs a decision)", w, Role::Queued, t));
    }
    out
}

/// Detail block of one node (right column at 200 columns, top of the node panel): the full values of every
/// column the tables may drop or cut.
fn node_detail_lines(nd: &NodeRow, w: usize, c: &Ctx) -> Lines {
    let t = c.theme;
    let rr = cov_role(nd);
    let l = |s: String, r: Role| text_line(s, w, r, t);
    let cov = cov_text(nd);
    let mut v = vec![
        l(format!("node {}", nd.id), Role::Label),
        Line::from(vec![
            span(cl("coverage "), Role::Label, t),
            span(cov.clone(), rr, t),
            span(
                cl(format!(
                    "  target {}  spec box {}  phase {}  attempt ended {}",
                    nd.target,
                    nd.spec_box.as_ref().map_or("-", Clean::as_str),
                    nd.phase,
                    if nd.attempt_ended { "yes" } else { "no" }
                ))
                .fit(w.saturating_sub(9 + cov.width())),
                Role::Normal,
                t,
            ),
        ]),
        l(
            format!(
                "certification asked {}  reached {}  ({}){}",
                nd.cert_asked.as_ref().map_or("-", Clean::as_str),
                nd.cert_reached.as_ref().map_or("-", Clean::as_str),
                if nd.at_level() {
                    "at the level asked"
                } else {
                    "not yet at the level asked"
                },
                if nd.provisional {
                    "   PROVISIONAL: closed against measured hand-down ranges (rechecked when proved)"
                } else {
                    ""
                }
            ),
            if nd.provisional {
                Role::Warn
            } else if nd.at_level() {
                Role::Ok
            } else {
                Role::Normal
            },
        ),
        l(
            format!(
                "closure  rounds {}  defect {}  closure keys {}  rules used {} of {} ({})",
                cellv(nd.closure_rounds),
                nd.defect_keys
                    .map(thousands)
                    .unwrap_or_else(|| "not checked yet".into()),
                cellv(nd.closure_keys),
                cellv(nd.rules_used),
                cellv(nd.rules_total),
                percent(nd.used_share)
            ),
            defect_role(nd.defect_keys, final_phase(nd)),
        ),
        l(
            if nd.economy_measured {
                format!(
                    "economy  rules {} -> {} (superseded {})  RHS terms {} -> {}",
                    cellv(nd.rules_before),
                    cellv(nd.rules_after),
                    cellv(nd.superseded),
                    cellv(nd.rhs_before),
                    cellv(nd.rhs_after)
                )
            } else {
                "economy  not measured".to_string()
            },
            Role::Normal,
        ),
        l(
            format!(
                "terminals {} of bound {} (candidates {})  items open {}  CPU {} h",
                cellv(nd.terminals),
                cellv(nd.terminal_bound),
                cellv(nd.terminal_candidates),
                cellv(nd.items_open),
                cpu_hours(nd.cpu_seconds)
            ),
            Role::Terminal,
        ),
        l(
            format!(
                "evidence geometry {}  tested {}  proved {}  (per-key proved {})",
                nd.accepted_by_evidence[0],
                nd.accepted_by_evidence[1],
                nd.accepted_by_evidence[2],
                percent(nd.proved_share)
            ),
            Role::Normal,
        ),
        l(
            format!(
                "blockers full: {}   box: {}",
                nd.blockers_full.as_ref().map_or("-", Clean::as_str),
                nd.blockers_box.as_ref().map_or("-", Clean::as_str)
            ),
            if nd.blockers_full.is_some() || nd.blockers_box.is_some() {
                Role::Warn
            } else {
                Role::Normal
            },
        ),
    ];
    if nd.preferred.is_some() {
        v.push(l(
            format!(
                "preferred terminals (a wish, not a requirement): {}   other terminals: {}",
                nd.preferred
                    .as_ref()
                    .map(|p| format!(
                        "used {} of {}, not needed {}",
                        cellv(p.used),
                        cellv(p.total),
                        cellv(p.not_needed)
                    ))
                    .unwrap_or_default(),
                match (&nd.preferred, nd.terminals) {
                    (Some(p), Some(t)) => p
                        .used
                        .map_or("?".into(), |u| t.saturating_sub(u).to_string()),
                    _ => "?".into(),
                }
            ),
            Role::Terminal,
        ));
    }
    if let Some(r) = &nd.reference {
        v.push(l(
            format!(
                "reference (information only) {}: terminals {}  rules {}  RHS terms {}  {}",
                r.name.as_ref().map_or("existing ledger", Clean::as_str),
                cellv(r.terminals),
                cellv(r.rules),
                cellv(r.rhs_terms),
                r.status.as_ref().map_or("", Clean::as_str)
            ),
            Role::Queued,
        ));
    }
    let phases: Vec<String> = PHASES
        .iter()
        .zip(nd.items_by_phase)
        .filter(|(_, n)| *n > 0)
        .map(|(p, n)| format!("{p} {n}"))
        .collect();
    v.push(l(
        format!(
            "items by state: {}",
            if phases.is_empty() {
                "-".into()
            } else {
                phases.join(", ")
            }
        ),
        Role::Normal,
    ));
    v.push(l(
        format!("open by rung: {}", by_rung_text(nd)),
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
fn tiny(vm: &ViewModel, w: usize, h: usize, c: &Ctx) -> Lines {
    let line = super::plain::status_line(vm, c.app);
    let mut out: Lines = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    for chunk in chars.chunks(w.max(1)) {
        out.push(Line::from(span(
            cl(chunk.iter().collect::<String>()).fit(w),
            banner_role(vm.banner),
            c.theme,
        )));
    }
    out.extend(banners(vm, w, c));
    out.push(text_line(
        "enlarge to 80x24 for the dashboard",
        w,
        Role::Queued,
        c.theme,
    ));
    out.truncate(h.max(1));
    out
}

/// The header block: the five-second answers first, then the details.
fn header_block(vm: &ViewModel, w: usize, c: &Ctx) -> Lines {
    let mut v = vec![header_line(vm, w, c)];
    v.extend(banners(vm, w, c));
    v.push(progress_line(vm, w, c));
    v.push(eta_line(vm, w, c));
    v.push(next_line(vm, w, c));
    v.push(resources_line(vm, w, c));
    v.push(cert_line(vm, w, c));
    v.push(items_line(vm, w, c));
    v.push(rules_line(vm, w, c));
    v
}

fn compact(vm: &ViewModel, w: usize, h: usize, c: &Ctx) -> Lines {
    let mut v = header_block(vm, w, c);
    let ev_rows = 3usize;
    let fixed = v.len() + 1 + 1 + ev_rows + 1;
    let avail = h.saturating_sub(fixed).max(2);
    let tab = c.app.compact;
    let title = match tab {
        CompactTable::Nodes => format!("Nodes ({})", vm.nodes.len()),
        CompactTable::Jobs => jobs_title(vm),
        other => other.title().to_string(),
    };
    v.push(rule(&title, "Tab: next table", w, c));
    let mut body = match tab {
        CompactTable::Ladder => ladder_table(vm, w, c),
        CompactTable::Nodes => nodes_table(vm, w, avail, WidthClass::Compact, c),
        CompactTable::Jobs => jobs_table(vm, w, avail, false, 0, c),
        CompactTable::Events => event_lines(&vm.events, w, avail, 0, false, c),
        CompactTable::Attention => attention_lines(&vm.attention, w, avail, true, c),
    };
    body.truncate(avail);
    let spare = avail - body.len();
    v.extend(body);
    v.push(rule("Recent events", "e: all", w, c));
    if tab == CompactTable::Events {
        v.extend(std::iter::repeat_n(Line::default(), ev_rows + spare));
    } else {
        let ev = ev_rows + spare;
        v.extend(
            event_lines(&vm.events, w, ev, 0, false, c)
                .into_iter()
                .take(ev),
        );
    }
    v.truncate(h.saturating_sub(1));
    while v.len() < h.saturating_sub(1) {
        v.push(Line::default());
    }
    v.push(footer(vm, w, WidthClass::Compact, c));
    v
}

/// The work list by ladder level beside the rules and certificates panel (the ladder takes its natural width; the
/// panel gets the rest).
fn ladder_rules(vm: &ViewModel, w: usize, c: &Ctx) -> Lines {
    let lw = ladder_width(vm, c).clamp(30.min(w), w);
    let rw = w.saturating_sub(lw + 2);
    let mut left = vec![rule("Work list by ladder level", "", lw, c)];
    left.extend(ladder_table(vm, lw, c));
    if rw >= 30 {
        let mut right = vec![rule("Rules and certificates", "", rw, c)];
        right.extend(rules_panel(vm, rw, c));
        hjoin(left, lw, right, 2)
    } else {
        left
    }
}

/// Rows for the nodes table, the jobs table and the events, from what is left (each gets what it needs when it can;
/// the remainder goes where rows are missing, events last).
fn allot(rest: usize, need: [usize; 3], min: [usize; 3]) -> [usize; 3] {
    let mut got = min;
    let mut left = rest.saturating_sub(min.iter().sum());
    // a fair third first, then fill in order
    for i in 0..3 {
        let fair = (rest / 3).saturating_sub(got[i]);
        let add = need[i].saturating_sub(got[i]).min(fair).min(left);
        got[i] += add;
        left -= add;
    }
    for i in 0..3 {
        let add = need[i].saturating_sub(got[i]).min(left);
        got[i] += add;
        left -= add;
    }
    got[2] += left;
    got
}

fn default_screen(vm: &ViewModel, w: usize, h: usize, c: &Ctx, class: WidthClass) -> Lines {
    let mut v = header_block(vm, w, c);
    let wide = class == WidthClass::Wide;
    // the middle block: ladder | rules (| selected node + attention at 200 columns)
    let middle = if wide {
        let lw = (ladder_width(vm, c) + 2 + 62)
            .min(w.saturating_sub(40))
            .max(60);
        let rw = w.saturating_sub(lw + 2);
        let left = ladder_rules(vm, lw, c);
        let mut right = Vec::new();
        if let Some(nd) = vm
            .nodes
            .get(c.app.selected_node.min(vm.nodes.len().saturating_sub(1)))
        {
            right.push(rule(&format!("{} (selected; n/N)", nd.id), "", rw, c));
            right.extend(node_detail_lines(nd, rw, c));
        }
        right.push(rule(
            &format!("Attention ({})", vm.attention.len()),
            "a: all",
            rw,
            c,
        ));
        right.extend(attention_lines(&vm.attention, rw, 6, true, c));
        let cap = left.len().max(right.len()).min(left.len() + 12);
        let mut j = hjoin(left, lw, right, 2);
        j.truncate(cap);
        j
    } else {
        ladder_rules(vm, w, c)
    };
    let jobs_n = vm.jobs.list.as_ref().map_or(0, Vec::len);
    let fixed = v.len() + 1 + middle.len() + 1 + 1 + 1;
    let rest = h.saturating_sub(fixed);
    let [nodes_rows, jobs_rows, ev_rows] = allot(
        rest,
        [vm.nodes.len() + 1, jobs_n + 1, vm.events.len().max(1)],
        [2, 2, 1],
    );
    v.push(rule(
        &format!("Nodes ({})", vm.nodes.len()),
        "Enter: node detail",
        w,
        c,
    ));
    v.extend(nodes_table(vm, w, nodes_rows, class, c));
    v.extend(middle);
    v.push(rule(&jobs_title(vm), "j: all jobs", w, c));
    v.extend(jobs_table(vm, w, jobs_rows, wide, 0, c));
    v.push(rule("Recent events", "e: all events", w, c));
    v.extend(event_lines(&vm.events, w, ev_rows, 0, wide, c));
    v.truncate(h.saturating_sub(1));
    while v.len() < h.saturating_sub(1) {
        v.push(Line::default());
    }
    v.push(footer(vm, w, class, c));
    v
}

fn item_table(c: &Ctx, w: usize, n: usize) -> Lines {
    let items = c.app.visible_items();
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
                _ => Role::Normal,
            };
            vec![
                (i.item.clone(), Role::Normal),
                (i.label.clone(), Role::Normal),
                (i.kind.clone(), Role::Queued),
                (i.rung.clone(), Role::Label),
                (i.state.clone(), r),
                (i.why.clone(), Role::Normal),
                (cl(format!("{:.1}", i.cpu_seconds)), Role::Normal),
                (cl(i.jobs.to_string()), Role::Normal),
                (cl(i.updated_seq.to_string()), Role::Queued),
            ]
        })
        .collect();
    let wi = |k: usize| rows.iter().map(move |r| r[k].0.width());
    let cols = [
        col("item", wi(0), 4, 24, Align::L, 0).mid(),
        col("label", wi(1), 5, 28, Align::L, 0).mid(),
        col("kind", wi(2), 4, 12, Align::L, 3),
        col("rung", wi(3), 4, 12, Align::L, 0),
        col("state", wi(4), 5, 18, Align::L, 0),
        col("why", wi(5), 3, 60, Align::L, 1),
        col("cpu s", wi(6), 5, 10, Align::R, 2),
        col("jobs", wi(7), 4, 6, Align::R, 2),
        col("seq", wi(8), 3, 10, Align::R, 4),
    ];
    table(&cols, &rows, w, Some(sel - lo), c)
}

fn panel_screen(vm: &ViewModel, w: usize, h: usize, c: &Ctx) -> Lines {
    let t = c.theme;
    let mut v = vec![header_line(vm, w, c)];
    v.extend(banners(vm, w, c));
    let room = |v: &Lines| h.saturating_sub(v.len() + 2).max(1);
    match c.app.panel {
        Panel::Node => {
            let id = c
                .app
                .node_items
                .as_ref()
                .map(|(id, _)| id.clone())
                .unwrap_or_default();
            v.push(rule(&format!("Node {id}"), "q: back", w, c));
            if let Some(nd) = vm.nodes.iter().find(|n| n.id == id) {
                v.extend(node_detail_lines(nd, w, c));
            }
            let shown = c.app.visible_items().len();
            let filter = c.app.item_filter.map(|k| PHASES[k]).unwrap_or("all");
            let all = match &c.app.node_items {
                Some((_, Slot::Ready(Ok(v)))) => v.len(),
                _ => 0,
            };
            v.push(rule(
                &format!("Items ({shown} of {all}; state: {filter})"),
                "1-9,0 filter  Enter/w why",
                w,
                c,
            ));
            match &c.app.node_items {
                Some((_, Slot::Ready(Err(e)))) => {
                    v.push(text_line(format!("! {e}"), w, Role::Warn, t))
                }
                Some((_, Slot::Loading)) => v.push(text_line(
                    "loading the item list (read in the background) ...",
                    w,
                    Role::Running,
                    t,
                )),
                _ => {
                    let n = room(&v).saturating_sub(1).max(1);
                    v.extend(item_table(c, w, n));
                }
            }
        }
        Panel::Item => {
            let id = c
                .app
                .item_detail
                .as_ref()
                .map(|(id, _)| id.clone())
                .unwrap_or_default();
            v.push(rule(&format!("Item {id} (why)"), "q: back", w, c));
            match c.app.item_detail.as_ref().map(|(_, d)| d) {
                Some(Slot::Ready(Ok(val))) => v.extend(item_lines(val, w, c)),
                Some(Slot::Ready(Err(e))) => v.push(text_line(format!("! {e}"), w, Role::Warn, t)),
                Some(Slot::Loading) => v.push(text_line(
                    "loading the item detail (read in the background) ...",
                    w,
                    Role::Running,
                    t,
                )),
                None => {}
            }
        }
        Panel::Jobs => {
            v.push(rule(&jobs_title(vm), "q: back", w, c));
            let n = room(&v);
            v.extend(jobs_table(vm, w, n, w >= 160, c.app.scroll, c));
        }
        Panel::Events => {
            v.push(rule(
                &format!("Recent events ({})", vm.events.len()),
                "q: back",
                w,
                c,
            ));
            let n = room(&v);
            let skip = c.app.scroll.min(vm.events.len().saturating_sub(1));
            v.extend(event_lines(&vm.events, w, n, skip, true, c));
        }
        Panel::Attention => {
            v.push(rule(
                &format!("Attention ({})", vm.attention.len()),
                "q: back",
                w,
                c,
            ));
            let n = room(&v);
            v.extend(attention_lines(&vm.attention, w, n, true, c));
        }
        Panel::Help => {
            v.push(rule("Help", "q: back", w, c));
            for s in help_text(vm) {
                v.push(text_line(s, w, Role::Normal, t));
            }
            if !vm.problems.is_empty() {
                v.push(rule("Problems with the inputs", "", w, c));
                for p in &vm.problems {
                    v.push(text_line(format!("! {p}"), w, Role::Fail, t));
                }
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

fn item_lines(d: &serde_json::Value, w: usize, c: &Ctx) -> Lines {
    let t = c.theme;
    let g = |p: &str| -> String {
        match d.pointer(p) {
            None | Some(serde_json::Value::Null) => "-".into(),
            Some(serde_json::Value::String(s)) => s.clone(),
            Some(v) => v.to_string(),
        }
    };
    let mut v = vec![
        Line::from(vec![
            span(cl("why "), Role::Label, t),
            span(cl(g("/why")).fit(w.saturating_sub(4)), Role::Warn, t),
        ]),
        text_line(
            format!(
                "node {}  label {}  kind {}  rung {}  phase {} {}",
                g("/record/node"),
                g("/record/label"),
                g("/record/kind"),
                g("/record/rung"),
                g("/record/phase/phase"),
                g("/record/phase/detail")
            ),
            w,
            Role::Normal,
            t,
        ),
        text_line(
            format!(
                "parent {}  weight {}  created seq {}  updated seq {}  cpu {} ms  terminal keys {}",
                g("/record/parent"),
                g("/record/weight"),
                g("/record/created_seq"),
                g("/record/updated_seq"),
                g("/record/cpu_ms"),
                g("/record/terminal_keys")
            ),
            w,
            Role::Normal,
            t,
        ),
        text_line(
            format!("accepted {}", g("/record/accepted")),
            w,
            Role::Ok,
            t,
        ),
        text_line(
            format!("retired {}", g("/record/retired")),
            w,
            Role::Queued,
            t,
        ),
        text_line(
            format!("economy {}", g("/record/economy")),
            w,
            Role::Normal,
            t,
        ),
        text_line(
            format!(
                "stale from {}  input {}",
                g("/record/stale_from"),
                g("/record/stale_input")
            ),
            w,
            Role::Warn,
            t,
        ),
    ];
    v.push(Line::from(span(cl("jobs:"), Role::Label, t)));
    if let Some(a) = d.get("jobs").and_then(|j| j.as_array()) {
        for j in a.iter().take(50) {
            let g2 = |p: &str| -> String {
                match j.pointer(p) {
                    None | Some(serde_json::Value::Null) => "-".into(),
                    Some(serde_json::Value::String(s)) => s.clone(),
                    Some(x) => x.to_string(),
                }
            };
            v.push(text_line(
                format!(
                    "  {} attempt {}  {} {}  cpu {} s  run dir {}",
                    g2("/job"),
                    g2("/attempt"),
                    g2("/state/state"),
                    g2("/state/class"),
                    g2("/cpu_seconds"),
                    g2("/run_dir")
                ),
                w,
                Role::Normal,
                t,
            ));
        }
    }
    v.push(text_line(
        format!("children {}", g("/children")),
        w,
        Role::Normal,
        t,
    ));
    v
}

fn help_text(vm: &ViewModel) -> Vec<String> {
    vec![
        "VIEW   Tab next table (80 cols)   n/N or arrows select   Enter/f/i node detail   w why (item)".into(),
        "       j running jobs   e events   a attention   ? help   q/Esc back   PgUp/PgDn/Home/End scroll".into(),
        "STOP   Ctrl-C once: drain (no new jobs; running ones finish or stop after the grace, recorded interrupted)".into(),
        "       Ctrl-C twice: forced stop, still safe   third (after a pause): leave the dashboard   resume: same command".into(),
        "       (in a read-only viewer Ctrl-C and q only close the viewer)".into(),
        "CONTROL  pause / resume / drain / retry / skip / terminal / workers: rr_campaign <verb> in another pane".into(),
        "COLOURS  green FULL/ok   blue [BOXED box] (closed inside the user's box)   magenta terminals   cyan RUN".into(),
        "         yellow escalated / attention / STALE / near a cap / defect on a running node   red failed / rejected /".into(),
        "         OPEN with blockers / defect after the closure check   dim: estimates, old values".into(),
        "VALUES   - none (0)   ? not known   a number followed by ? contradicts another number (listed below)".into(),
        "PROGRESS nodes FULL + BOXED are closed; items done = accepted + escalated + terminal + retired of all items".into(),
        "COVERAGE FULL = every key of the sector reduces; BOXED = closed inside the box GIVEN BY THE USER; OPEN = not closed".into(),
        "CERTIFIED nodes at the certification level THEIR spec asks (asked/got column); rules by the level that certifies".into(),
        "         them; proved% = accepted items at identity_proved (per-key proof), a detail".into(),
        "DEFECT   keys reached by the closure that are neither covered, nor terminals, nor handed down".into(),
        "TERMINALS declared (candidates waiting, bound per node); pref = the user's preferred terminals used / given (a wish)".into(),
        "RULES    rules used on the node's closure; used% = used / rules of the node's dispatch (economy, R31)".into(),
        format!(
            "ETA      {}; items that escalations create later are not counted (the range is early)",
            vm.eta.estimator.as_ref().map_or("remaining items x per-rung CPU quantiles / parallelism", Clean::as_str)
        ),
        format!(
            "STALE    the campaign says running but no heartbeat for > {} s ({})",
            vm.stale_after_s as u64,
            match vm.heartbeat_source {
                HeartbeatSource::Field => "supervisor heartbeat field",
                _ => "no heartbeat field: inferred from file times",
            }
        ),
        "DATA     progress/*.json read in the background; drill-down from a read-only ledger reduction".into(),
    ]
}

/// Builds every line of the frame.
pub fn screen_lines(vm: &ViewModel, w: u16, h: u16, c: &Ctx) -> Lines {
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
    v
}

/// Draws the frame.
pub fn draw(frame: &mut Frame, vm: &ViewModel, c: &Ctx) {
    let area: Rect = frame.area();
    let lines = screen_lines(vm, area.width, area.height, c);
    frame.render_widget(Paragraph::new(lines), area);
}
