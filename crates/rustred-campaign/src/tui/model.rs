//! The dashboard's VIEW-MODEL: plain structs filled by ONE adapter function, [`build`], from the progress files.
//! Only this file knows progress field names; a schema addition touches this file alone. Unknown values stay
//! `None` (shown as `?` / `-`), never 0.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use super::format::clean;
use super::source::{FileRead, JobRow, RawSnapshot};

/// Item phases in display order (as-built `ItemPhase::TAGS`).
pub const PHASES: [&str; 12] = [
    "open",
    "in_progress",
    "produced",
    "accepted",
    "rejected",
    "escalated",
    "terminal_candidate",
    "terminal",
    "stale",
    "retired",
    "attention",
    "skipped",
];

/// Phases counted as open work (`ItemPhase::is_open`).
pub fn phase_is_open(p: &str) -> bool {
    !matches!(p, "accepted" | "escalated" | "terminal" | "retired")
}

/// The headline state word.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Banner {
    NoData,
    Unreadable,
    Starting,
    Running,
    Draining,
    Stopped,
    Finished,
    Stale,
    Unknown,
}

impl Banner {
    pub fn word(self) -> &'static str {
        match self {
            Banner::NoData => "NO DATA",
            Banner::Unreadable => "UNREADABLE",
            Banner::Starting => "STARTING",
            Banner::Running => "RUNNING",
            Banner::Draining => "DRAINING",
            Banner::Stopped => "STOPPED",
            Banner::Finished => "FINISHED",
            Banner::Stale => "STALE",
            Banner::Unknown => "UNKNOWN",
        }
    }
}

/// FULL / BOXED / OPEN (2.4.15).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Coverage {
    Full,
    Boxed,
    Open,
    Unknown,
}

impl Coverage {
    pub fn word(self) -> &'static str {
        match self {
            Coverage::Full => "FULL",
            Coverage::Boxed => "BOXED",
            Coverage::Open => "OPEN",
            Coverage::Unknown => "?",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ResourcesView {
    pub cpus_in_use: Option<u64>,
    pub cpus_allowed: Option<u64>,
    pub live_jobs: Option<u64>,
    pub rss_bytes: Option<u64>,
    pub reserved_bytes: Option<u64>,
    pub mem_cap_bytes: Option<u64>,
    pub host_available_bytes: Option<u64>,
    pub host_floor_bytes: Option<u64>,
    pub disk_free_bytes: Option<u64>,
    pub sampled_utc: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct LadderRow {
    pub rung: String,
    /// Counts per item phase, indexed like [`PHASES`].
    pub counts: [u64; 12],
    pub jobs: Option<u64>,
    pub median_s: Option<f64>,
}

impl LadderRow {
    pub fn open(&self) -> u64 {
        PHASES
            .iter()
            .zip(self.counts)
            .filter(|(p, _)| phase_is_open(p))
            .map(|(_, c)| c)
            .sum()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct EtaView {
    pub remaining_items: Option<u64>,
    pub wall_s: Option<(f64, f64)>,
    pub cpu_s: Option<(f64, f64)>,
    pub parallelism: Option<u64>,
    pub pooled_levels: Vec<String>,
    pub reason: Option<String>,
    pub estimator: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NodeRow {
    pub id: String,
    pub coverage: Coverage,
    pub certified: bool,
    pub phase: String,
    /// `full`, or the box id of a BOXED target.
    pub target: String,
    pub spec_box: Option<String>,
    pub attempt_ended: bool,
    pub faces_closed: Option<u64>,
    pub faces_total: Option<u64>,
    pub items_open: Option<u64>,
    /// Open items per rung (non-zero only).
    pub open_by_rung: Vec<(String, u64)>,
    pub items_by_phase: [u64; 12],
    pub terminals: Option<u64>,
    pub terminal_bound: Option<u64>,
    pub terminal_candidates: Option<u64>,
    pub defect_keys: Option<u64>,
    pub closure_rounds: Option<u64>,
    pub closure_keys: Option<u64>,
    pub rules_used: Option<u64>,
    pub rules_total: Option<u64>,
    pub used_share: Option<f64>,
    pub rules_after: Option<u64>,
    pub rules_before: Option<u64>,
    pub rhs_after: Option<u64>,
    pub rhs_before: Option<u64>,
    pub superseded: Option<u64>,
    pub certified_share: Option<f64>,
    pub accepted_by_evidence: [u64; 3],
    pub cpu_seconds: Option<f64>,
    pub stale_reason: Option<String>,
    pub blockers_full: Option<String>,
    pub blockers_box: Option<String>,
    /// Why this node file is not usable (absent, invalid, unknown schema).
    pub problem: Option<String>,
    /// Closed against MEASURED hand-down ranges (R41 P-2): closed, labelled provisional, rechecked later.
    pub provisional: bool,
    /// Certification level asked by the spec (`replay`, `per-key`, ...) and reached.
    pub cert_asked: Option<String>,
    pub cert_reached: Option<String>,
    /// The user's optional PREFERRED terminals (R41 P-4): total in the wish list, used, not needed.
    pub preferred: Option<Preferred>,
    /// Read-only REFERENCE numbers (an existing ledger): comparison only, never an error.
    pub reference: Option<Reference>,
}

/// Preferred terminals (a preference, never a requirement).
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Preferred {
    pub total: Option<u64>,
    pub used: Option<u64>,
    pub not_needed: Option<u64>,
}

/// Read-only comparison numbers from an existing ledger.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Reference {
    pub name: Option<String>,
    pub terminals: Option<u64>,
    pub rules: Option<u64>,
    pub rhs_terms: Option<u64>,
    pub status: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum EventTag {
    Ok,
    Esc,
    Fail,
    Term,
    Boxed,
    Info,
}

impl EventTag {
    pub fn word(self) -> &'static str {
        match self {
            EventTag::Ok => "ok",
            EventTag::Esc => "esc",
            EventTag::Fail => "FAIL",
            EventTag::Term => "T",
            EventTag::Boxed => "box",
            EventTag::Info => ".",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EventRow {
    pub seq: u64,
    pub utc: String,
    pub kind: String,
    pub text: String,
    pub tag: EventTag,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Severity {
    Decision,
    Warning,
    Error,
}

impl Severity {
    pub fn word(self) -> &'static str {
        match self {
            Severity::Decision => "decision",
            Severity::Warning => "warning",
            Severity::Error => "error",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AttentionRow {
    pub id: String,
    pub code: String,
    pub severity: Severity,
    pub text: String,
    pub options: Vec<String>,
    /// True when computed by the dashboard (no attention file in the progress schema yet).
    pub derived: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct JobsView {
    pub total: Option<u64>,
    pub running: Option<u64>,
    pub launching: Option<u64>,
    pub queued: Option<u64>,
    pub by_state: BTreeMap<String, u64>,
    pub settled_by_class: BTreeMap<String, u64>,
    pub cap: Option<u64>,
    /// Rows: from `campaign.json` `jobs.list` (plan field), else from the source's ledger reduction.
    pub list: Option<Vec<JobRow>>,
    pub list_origin: &'static str,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct RulesView {
    pub accepted: Option<u64>,
    pub rejected: Option<u64>,
    pub rejected_by_reason: BTreeMap<String, u64>,
    pub upgrades: Option<u64>,
    pub retired: Option<u64>,
    /// geometry_accepted, identity_tested, identity_proved
    pub by_evidence: [Option<u64>; 3],
    pub rules_before: Option<u64>,
    pub rules_after: Option<u64>,
    pub rhs_before: Option<u64>,
    pub rhs_after: Option<u64>,
}

/// Everything the screens show.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ViewModel {
    pub origin: String,
    pub banner: Banner,
    /// Raw campaign phase word.
    pub phase: Option<String>,
    pub campaign_id: Option<String>,
    pub title: Option<String>,
    pub now_utc: String,
    pub seq: Option<u64>,
    pub last_event_utc: Option<String>,
    pub written_utc: Option<String>,
    pub heartbeat_age_s: Option<f64>,
    pub stale_after_s: f64,
    /// Seconds since the supervisor started (`up`) or since the campaign was created (`age`).
    pub elapsed_s: Option<f64>,
    pub elapsed_label: &'static str,
    pub sessions: Option<u64>,
    pub crash_recoveries: Option<u64>,
    pub lock_takeovers: Option<u64>,
    pub torn_tail_recoveries: Option<u64>,
    pub pending_operations: Option<u64>,
    pub unsettled_jobs: Option<u64>,
    pub grace_left_s: Option<f64>,
    pub resources: ResourcesView,
    pub nodes_total: Option<u64>,
    pub nodes_full: Option<u64>,
    pub nodes_boxed: Option<u64>,
    pub nodes_open: Option<u64>,
    pub nodes_certified: Option<u64>,
    pub nodes_by_phase: BTreeMap<String, u64>,
    pub items_total: Option<u64>,
    pub items_by_phase: [Option<u64>; 12],
    pub items_open: Option<u64>,
    pub ladder: Vec<LadderRow>,
    pub rules: RulesView,
    pub terminals_declared: Option<u64>,
    pub terminal_candidates: Option<u64>,
    pub terminal_bound: Option<u64>,
    pub certified_share: Option<f64>,
    pub certified_proved: Option<u64>,
    pub certified_accepted: Option<u64>,
    pub certified_basis: Option<String>,
    pub defect_keys: Option<u64>,
    pub nodes_checked: Option<u64>,
    pub nodes_with_defect: Option<u64>,
    pub faces_closed: Option<u64>,
    pub faces_total: Option<u64>,
    pub cost_cpu_s: Option<f64>,
    pub budget_cpu_hours: Option<f64>,
    /// Campaign mode (`generate`, `optimise`, ...); `optimise` shows economy before / after.
    pub mode: Option<String>,
    /// Certification level asked by the spec for the campaign (when the campaign file states it).
    pub cert_asked: Option<String>,
    pub preferred: Option<Preferred>,
    pub eta: EtaView,
    pub nodes: Vec<NodeRow>,
    pub jobs: JobsView,
    /// Newest first.
    pub events: Vec<EventRow>,
    pub attention: Vec<AttentionRow>,
    /// Problems with the inputs (unreadable files, unknown schemas, last good copies in use).
    pub problems: Vec<String>,
}

fn u(v: &Value, p: &str) -> Option<u64> {
    let x = v.pointer(p)?;
    x.as_u64().or_else(|| {
        x.as_f64()
            .filter(|f| f.is_finite() && *f >= 0.0)
            .map(|f| f as u64)
    })
}

fn f(v: &Value, p: &str) -> Option<f64> {
    v.pointer(p)?.as_f64().filter(|x| x.is_finite())
}

fn s(v: &Value, p: &str) -> Option<String> {
    v.pointer(p)?.as_str().map(clean)
}

fn nonzero(x: Option<u64>) -> Option<u64> {
    x.filter(|v| *v > 0)
}

fn counts_map(v: Option<&Value>) -> BTreeMap<String, u64> {
    let mut m = BTreeMap::new();
    if let Some(o) = v.and_then(Value::as_object) {
        for (k, x) in o.iter().take(256) {
            if let Some(n) = x.as_u64() {
                m.insert(clean(k), n);
            }
        }
    }
    m
}

fn phase_array(v: Option<&Value>) -> [u64; 12] {
    let mut a = [0u64; 12];
    if let Some(o) = v.and_then(Value::as_object) {
        for (i, p) in PHASES.iter().enumerate() {
            a[i] = o.get(*p).and_then(Value::as_u64).unwrap_or(0);
        }
    }
    a
}

fn iso_s(iso: &str) -> Option<f64> {
    crate::clock::UtcMillis::parse_iso(iso).map(|m| m.0 as f64 / 1000.0)
}

/// Sort key of a rung word: `L0` < `L1` < ... < `CERT` < others.
fn rung_key(r: &str) -> (u32, u32, String) {
    match r.strip_prefix('L').and_then(|n| n.parse::<u32>().ok()) {
        Some(n) => (0, n, String::new()),
        None if r == "CERT" => (1, 0, String::new()),
        None => (2, 0, r.to_string()),
    }
}

fn blockers(v: Option<&Value>) -> Option<String> {
    let b = v.filter(|x| x.is_object())?;
    let n = |k: &str| b.get(k).and_then(Value::as_u64).unwrap_or(0);
    Some(format!(
        "{} faces, {} loci, {} keys",
        n("faces"),
        n("loci"),
        n("keys")
    ))
}

fn preferred(v: Option<&Value>) -> Option<Preferred> {
    let p = v.filter(|x| x.is_object())?;
    Some(Preferred {
        total: u(p, "/total"),
        used: u(p, "/used"),
        not_needed: u(p, "/not_needed"),
    })
}

fn reference(v: Option<&Value>) -> Option<Reference> {
    let r = v.filter(|x| x.is_object())?;
    let out = Reference {
        name: s(r, "/name"),
        terminals: u(r, "/terminals"),
        rules: u(r, "/rules"),
        rhs_terms: u(r, "/rhs_terms"),
        status: s(r, "/status").or_else(|| s(r, "/comparison/status")),
    };
    (out.terminals.is_some()
        || out.rules.is_some()
        || out.rhs_terms.is_some()
        || out.name.is_some())
    .then_some(out)
}

fn event_tag(kind: &str, text: &str) -> EventTag {
    let t = text.to_ascii_lowercase();
    match kind {
        "item_accepted" | "item_upgraded" => EventTag::Ok,
        "item_escalated" | "terminal_proposed" | "stale_inputs" => EventTag::Esc,
        "item_rejected" | "item_failed" | "control_refused" => EventTag::Fail,
        "terminal_declared" => EventTag::Term,
        "job_finished" => {
            if t.contains("(failed")
                || t.contains("crash")
                || t.contains("killed")
                || t.contains("lost")
            {
                EventTag::Fail
            } else {
                EventTag::Info
            }
        }
        "node" | "node_closed" => {
            if t.contains("box") {
                EventTag::Boxed
            } else if t.contains("certified") || t.contains("closed") {
                EventTag::Ok
            } else if t.contains("open") && t.contains("declared") {
                EventTag::Fail
            } else {
                EventTag::Info
            }
        }
        _ if kind.contains("fail") || kind.contains("reject") => EventTag::Fail,
        _ => EventTag::Info,
    }
}

/// Options of the adapter.
#[derive(Clone, Copy, Debug)]
pub struct BuildOptions {
    pub now_s: f64,
    pub stale_after_s: f64,
}

fn node_row(id: &str, r: &FileRead) -> NodeRow {
    let mut row = NodeRow {
        id: clean(id),
        coverage: Coverage::Unknown,
        certified: false,
        phase: "?".into(),
        target: "?".into(),
        spec_box: None,
        attempt_ended: false,
        faces_closed: None,
        faces_total: None,
        items_open: None,
        open_by_rung: Vec::new(),
        items_by_phase: [0; 12],
        terminals: None,
        terminal_bound: None,
        terminal_candidates: None,
        defect_keys: None,
        closure_rounds: None,
        closure_keys: None,
        rules_used: None,
        rules_total: None,
        used_share: None,
        rules_after: None,
        rules_before: None,
        rhs_after: None,
        rhs_before: None,
        superseded: None,
        certified_share: None,
        accepted_by_evidence: [0; 3],
        cpu_seconds: None,
        stale_reason: None,
        blockers_full: None,
        blockers_box: None,
        problem: None,
        provisional: false,
        cert_asked: None,
        cert_reached: None,
        preferred: None,
        reference: None,
    };
    let v = match r {
        FileRead::Ok(v) => v,
        FileRead::Absent => {
            row.problem = Some("node file absent".into());
            return row;
        }
        FileRead::Invalid(w) => {
            row.problem = Some(clean(w));
            return row;
        }
        FileRead::UnknownSchema(w) => {
            row.problem = Some(clean(w));
            return row;
        }
    };
    row.coverage = match v.get("closure_kind").and_then(Value::as_str) {
        Some("full") => Coverage::Full,
        Some("boxed") => Coverage::Boxed,
        Some("open") => Coverage::Open,
        _ => Coverage::Unknown,
    };
    row.certified = v.get("certified").and_then(Value::as_bool).unwrap_or(false);
    row.phase = s(v, "/phase").unwrap_or_else(|| "?".into());
    row.target = match v.pointer("/target/target").and_then(Value::as_str) {
        Some("full") => "full".into(),
        Some("boxed") => s(v, "/target/box_id").unwrap_or_else(|| "box".into()),
        Some(o) => clean(o),
        None => "?".into(),
    };
    row.spec_box = s(v, "/spec_box");
    row.attempt_ended = v
        .get("attempt_ended")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    row.faces_total = u(v, "/faces_summary/total").or_else(|| u(v, "/faces/total"));
    row.faces_closed = u(v, "/faces_summary/closed").or_else(|| u(v, "/faces/closed"));
    row.items_open = u(v, "/items/open");
    row.items_by_phase = phase_array(v.pointer("/items/by_phase"));
    if let Some(o) = v.pointer("/items/by_level").and_then(Value::as_object) {
        let mut rows: Vec<(String, u64)> = o
            .iter()
            .take(64)
            .map(|(r, c)| {
                let a = phase_array(Some(c));
                let open: u64 = PHASES
                    .iter()
                    .zip(a)
                    .filter(|(p, _)| phase_is_open(p))
                    .map(|(_, n)| n)
                    .sum();
                (clean(r), open)
            })
            .filter(|(_, n)| *n > 0)
            .collect();
        rows.sort_by_key(|(r, _)| rung_key(r));
        row.open_by_rung = rows;
    }
    row.terminals = u(v, "/terminals/declared");
    row.terminal_bound = u(v, "/terminals/bound");
    row.terminal_candidates = u(v, "/terminals/candidates");
    row.defect_keys = u(v, "/closure/defect_keys");
    row.closure_rounds = u(v, "/closure/rounds");
    row.closure_keys = u(v, "/closure/closure_keys");
    row.rules_used = u(v, "/closure/rules_used");
    row.rules_total = u(v, "/closure/rules_total");
    row.used_share = f(v, "/closure/used_share");
    row.rules_after = u(v, "/economy/rules_after");
    row.rules_before = u(v, "/economy/rules_before");
    row.rhs_after = u(v, "/economy/after/rhs_terms");
    row.rhs_before = u(v, "/economy/before/rhs_terms");
    row.superseded = u(v, "/economy/superseded");
    let ev = v.get("accepted_by_evidence");
    let lv = |k: &str| {
        ev.and_then(|e| e.get(k))
            .and_then(Value::as_u64)
            .unwrap_or(0)
    };
    row.accepted_by_evidence = [
        lv("geometry_accepted"),
        lv("identity_tested"),
        lv("identity_proved"),
    ];
    let acc: u64 = row.accepted_by_evidence.iter().sum();
    row.certified_share = (acc > 0).then(|| row.accepted_by_evidence[2] as f64 / acc as f64);
    row.cpu_seconds = f(v, "/cpu_seconds");
    row.stale_reason = s(v, "/stale_reason");
    row.blockers_full = blockers(v.pointer("/blockers/full"));
    row.blockers_box = blockers(v.pointer("/blockers/box"));
    row.provisional = v
        .get("provisional")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || v.pointer("/closure/provisional")
            .and_then(Value::as_bool)
            .unwrap_or(false);
    row.cert_asked = s(v, "/certification/asked");
    row.cert_reached = s(v, "/certification/reached");
    row.preferred = preferred(v.pointer("/terminals/preferred"));
    row.reference = reference(v.get("reference"));
    row
}

/// THE adapter: progress files -> view-model.
pub fn build(raw: &RawSnapshot, o: BuildOptions) -> ViewModel {
    let mut problems: Vec<String> = raw.notes.iter().map(|n| clean(n)).collect();
    let null = Value::Null;
    let (c, banner0) = match &raw.campaign {
        FileRead::Ok(v) => (v, None),
        FileRead::Absent => {
            problems.push(format!(
                "progress/campaign.json absent (campaign not started yet, or wrong directory): {}",
                clean(&raw.origin)
            ));
            (&null, Some(Banner::NoData))
        }
        FileRead::Invalid(w) => {
            problems.push(format!("campaign.json: {}", clean(w)));
            (&null, Some(Banner::Unreadable))
        }
        FileRead::UnknownSchema(w) => {
            problems.push(format!("campaign.json: {}", clean(w)));
            (&null, Some(Banner::Unreadable))
        }
    };
    let phase = s(c, "/campaign/phase");
    let written = s(c, "/written_utc");
    let res_v = c.get("resources").unwrap_or(&null);
    let resources = ResourcesView {
        cpus_in_use: u(res_v, "/cpus_in_use"),
        cpus_allowed: nonzero(u(res_v, "/cpus_allowed")),
        live_jobs: u(res_v, "/live_jobs"),
        rss_bytes: u(res_v, "/measured_rss_bytes").or_else(|| u(res_v, "/campaign_rss_bytes")),
        reserved_bytes: u(res_v, "/reserved_bytes"),
        mem_cap_bytes: nonzero(
            u(res_v, "/mem_cap_bytes").or_else(|| u(res_v, "/campaign_mem_cap_bytes")),
        ),
        host_available_bytes: nonzero(u(res_v, "/host_mem_available_bytes")),
        host_floor_bytes: nonzero(u(res_v, "/host_floor_bytes")),
        disk_free_bytes: nonzero(u(res_v, "/disk_free_bytes")),
        sampled_utc: s(res_v, "/sampled_utc"),
    };
    // a resource block whose allowed CPUs and sample time are both absent was never measured
    let resources = if resources.cpus_allowed.is_none() && resources.sampled_utc.is_none() {
        ResourcesView {
            cpus_in_use: nonzero(resources.cpus_in_use),
            rss_bytes: nonzero(resources.rss_bytes),
            reserved_bytes: nonzero(resources.reserved_bytes),
            live_jobs: nonzero(resources.live_jobs),
            ..resources
        }
    } else {
        resources
    };
    // heartbeat: newest of the stamps we have
    let mut hb: Option<f64> = None;
    for t in [
        s(c, "/supervisor/heartbeat_utc").as_deref().and_then(iso_s),
        resources.sampled_utc.as_deref().and_then(iso_s),
        written.as_deref().and_then(iso_s),
        raw.campaign_mtime_s,
    ]
    .into_iter()
    .flatten()
    {
        hb = Some(hb.map_or(t, |h: f64| h.max(t)));
    }
    let heartbeat_age_s = hb.map(|h| (o.now_s - h).max(0.0));
    let live = matches!(phase.as_deref(), Some("starting" | "running" | "draining"));
    let stale = live && heartbeat_age_s.is_some_and(|a| a > o.stale_after_s);
    let banner = banner0.unwrap_or(if stale {
        Banner::Stale
    } else {
        match phase.as_deref() {
            Some("starting") => Banner::Starting,
            Some("running") => Banner::Running,
            Some("draining") => Banner::Draining,
            Some("stopped") => Banner::Stopped,
            Some("finished") => Banner::Finished,
            _ => Banner::Unknown,
        }
    });
    let (elapsed_s, elapsed_label) =
        match s(c, "/supervisor/started_utc").as_deref().and_then(iso_s) {
            Some(t) => (Some((o.now_s - t).max(0.0)), "up"),
            None => (
                s(c, "/campaign/created_utc")
                    .as_deref()
                    .and_then(iso_s)
                    .map(|t| (o.now_s - t).max(0.0)),
                "age",
            ),
        };
    // ladder
    let mut ladder: Vec<LadderRow> = Vec::new();
    if let Some(m) = c.pointer("/items/by_level").and_then(Value::as_object) {
        for (r, v) in m.iter().take(64) {
            ladder.push(LadderRow {
                rung: clean(r),
                counts: phase_array(Some(v)),
                jobs: u(c, &format!("/cost/by_level/{r}/jobs")),
                median_s: f(c, &format!("/cost/by_level/{r}/p50_s")),
            });
        }
    }
    if let Some(m) = c.pointer("/cost/by_level").and_then(Value::as_object) {
        for (r, _) in m.iter().take(64) {
            if !ladder.iter().any(|l| &l.rung == r) {
                ladder.push(LadderRow {
                    rung: clean(r),
                    counts: [0; 12],
                    jobs: u(c, &format!("/cost/by_level/{r}/jobs")),
                    median_s: f(c, &format!("/cost/by_level/{r}/p50_s")),
                });
            }
        }
    }
    ladder.sort_by_key(|l| rung_key(&l.rung));
    let ibp = c.pointer("/items/by_phase");
    let mut items_by_phase = [None; 12];
    if ibp.is_some_and(Value::is_object) {
        for (i, p) in PHASES.iter().enumerate() {
            items_by_phase[i] = ibp.and_then(|o| o.get(*p)).and_then(Value::as_u64);
        }
    }
    let items_open = if ibp.is_some_and(Value::is_object) {
        Some(
            PHASES
                .iter()
                .zip(items_by_phase)
                .filter(|(p, _)| phase_is_open(p))
                .map(|(_, n)| n.unwrap_or(0))
                .sum(),
        )
    } else {
        None
    };
    let ev = c.pointer("/rules/accepted_items_by_evidence");
    let lv = |k: &str| ev.and_then(|e| e.get(k)).and_then(Value::as_u64);
    let rules = RulesView {
        accepted: u(c, "/rules/accepted"),
        rejected: u(c, "/rules/rejected"),
        rejected_by_reason: counts_map(c.pointer("/rules/rejected_by_reason")),
        upgrades: u(c, "/rules/upgrades"),
        retired: u(c, "/economy/retired"),
        by_evidence: [
            lv("geometry_accepted"),
            lv("identity_tested"),
            lv("identity_proved"),
        ],
        rules_before: u(c, "/economy/rules_before"),
        rules_after: u(c, "/economy/rules_after"),
        rhs_before: u(c, "/economy/before/rhs_terms"),
        rhs_after: u(c, "/economy/after/rhs_terms"),
    };
    let pair = |p: &str| -> Option<(f64, f64)> {
        Some((f(c, &format!("{p}/low"))?, f(c, &format!("{p}/high"))?))
    };
    let eta = EtaView {
        remaining_items: u(c, "/eta/remaining_items"),
        wall_s: pair("/eta/wall_seconds"),
        cpu_s: pair("/eta/cpu_seconds"),
        parallelism: u(c, "/eta/parallelism"),
        pooled_levels: c
            .pointer("/eta/pooled_levels")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .take(16)
                    .filter_map(Value::as_str)
                    .map(clean)
                    .collect()
            })
            .unwrap_or_default(),
        reason: s(c, "/eta/reason"),
        estimator: s(c, "/eta/estimator"),
    };
    // nodes
    let mut nodes: Vec<NodeRow> = raw.nodes.iter().map(|(id, r)| node_row(id, r)).collect();
    for n in &nodes {
        if let Some(p) = &n.problem {
            problems.push(format!("node {}: {p}", n.id));
        }
    }
    nodes.sort_by(|a, b| a.id.cmp(&b.id));
    let faces_total: Option<u64> = if nodes.iter().any(|n| n.faces_total.is_some()) {
        Some(nodes.iter().filter_map(|n| n.faces_total).sum())
    } else {
        u(c, "/measures/faces/total")
    };
    let faces_closed: Option<u64> = if nodes.iter().any(|n| n.faces_closed.is_some()) {
        Some(nodes.iter().filter_map(|n| n.faces_closed).sum())
    } else {
        u(c, "/measures/faces/closed")
    };
    // jobs
    let by_state = counts_map(c.pointer("/jobs/by_state"));
    let list_from_file: Option<Vec<JobRow>> =
        c.pointer("/jobs/list").and_then(Value::as_array).map(|a| {
            a.iter()
                .take(1024)
                .map(|j| JobRow {
                    job: s(j, "/job").unwrap_or_default(),
                    node: s(j, "/node").unwrap_or_default(),
                    item: s(j, "/item").unwrap_or_default(),
                    rung: s(j, "/rung").unwrap_or_default(),
                    cpus: u(j, "/cpus").or_else(|| u(j, "/cpu").map(|_| 1)),
                    rss_bytes: u(j, "/rss_bytes"),
                    cap_bytes: u(j, "/budget_mem_bytes").or_else(|| u(j, "/cap_bytes")),
                    elapsed_s: f(j, "/seconds"),
                    budget_s: f(j, "/budget_seconds"),
                    heartbeat_age_s: s(j, "/heartbeat_utc")
                        .as_deref()
                        .and_then(iso_s)
                        .map(|t| (o.now_s - t).max(0.0)),
                    phase: s(j, "/progress/phase").or_else(|| s(j, "/phase")),
                    pid: u(j, "/pgid").or_else(|| u(j, "/pid")),
                    run_dir: s(j, "/run_dir"),
                })
                .collect()
        });
    let jobs = JobsView {
        total: u(c, "/jobs/total"),
        running: by_state
            .get("running")
            .copied()
            .or_else(|| u(c, "/jobs/running")),
        launching: by_state.get("launching").copied(),
        queued: by_state.get("queued").copied(),
        settled_by_class: counts_map(c.pointer("/jobs/settled_by_class")),
        by_state,
        cap: u(c, "/jobs/cap"),
        list_origin: if list_from_file.is_some() {
            "progress"
        } else {
            ""
        },
        list: list_from_file,
    };
    // events, newest first
    let events_v = match &raw.events {
        FileRead::Ok(v) => v.get("events").and_then(Value::as_array).cloned(),
        FileRead::Absent => None,
        FileRead::Invalid(w) | FileRead::UnknownSchema(w) => {
            problems.push(format!("events.json: {}", clean(w)));
            None
        }
    }
    .or_else(|| c.get("recent").and_then(Value::as_array).cloned())
    .unwrap_or_default();
    let mut events: Vec<EventRow> = events_v
        .iter()
        .rev()
        .take(256)
        .map(|e| {
            let kind = s(e, "/kind").unwrap_or_else(|| "?".into());
            let text = s(e, "/text").unwrap_or_default();
            EventRow {
                seq: u(e, "/seq").unwrap_or(0),
                utc: s(e, "/utc").unwrap_or_default(),
                tag: event_tag(&kind, &text),
                kind,
                text,
            }
        })
        .collect();
    events.sort_by_key(|e| std::cmp::Reverse(e.seq));
    let mut vm = ViewModel {
        origin: clean(&raw.origin),
        banner,
        phase,
        campaign_id: s(c, "/campaign_id"),
        title: s(c, "/title"),
        now_utc: crate::clock::UtcMillis((o.now_s * 1000.0) as i64).to_iso(),
        seq: u(c, "/ledger/seq"),
        last_event_utc: s(c, "/ledger/last_utc"),
        written_utc: written,
        heartbeat_age_s,
        stale_after_s: o.stale_after_s,
        elapsed_s,
        elapsed_label,
        sessions: u(c, "/campaign/sessions"),
        crash_recoveries: u(c, "/campaign/crash_recoveries"),
        lock_takeovers: u(c, "/campaign/lock_takeovers"),
        torn_tail_recoveries: u(c, "/campaign/torn_tail_recoveries"),
        pending_operations: u(c, "/campaign/pending_operations"),
        unsettled_jobs: u(c, "/campaign/unsettled_jobs"),
        grace_left_s: f(c, "/supervisor/grace_left_s"),
        resources,
        nodes_total: u(c, "/nodes/total"),
        nodes_full: u(c, "/nodes/by_closure/full"),
        nodes_boxed: u(c, "/nodes/by_closure/boxed"),
        nodes_open: u(c, "/nodes/by_closure/open"),
        nodes_certified: u(c, "/nodes/certified"),
        nodes_by_phase: counts_map(c.pointer("/nodes/by_phase")),
        items_total: u(c, "/items/total"),
        items_by_phase,
        items_open,
        ladder,
        rules,
        terminals_declared: u(c, "/terminals/declared"),
        terminal_candidates: u(c, "/terminals/candidates"),
        terminal_bound: u(c, "/terminals/bound_per_node_default"),
        certified_share: f(c, "/certified/share"),
        certified_proved: u(c, "/certified/identity_proved"),
        certified_accepted: u(c, "/certified/accepted"),
        certified_basis: s(c, "/certified/basis"),
        defect_keys: u(c, "/closure/defect_keys"),
        nodes_checked: u(c, "/closure/nodes_checked"),
        nodes_with_defect: u(c, "/closure/nodes_with_defect"),
        faces_closed,
        faces_total,
        cost_cpu_s: f(c, "/cost/cpu_seconds"),
        budget_cpu_hours: f(c, "/cost/budget_cpu_hours"),
        mode: s(c, "/campaign/mode").or_else(|| s(c, "/mode")),
        cert_asked: s(c, "/certification/asked"),
        preferred: preferred(c.pointer("/terminals/preferred")),
        eta,
        nodes,
        jobs,
        events,
        attention: Vec::new(),
        problems,
    };
    vm.attention = attention(c, raw, &vm);
    vm
}

fn attention(c: &Value, raw: &RawSnapshot, vm: &ViewModel) -> Vec<AttentionRow> {
    let mut out = Vec::new();
    // plan fields first (attention.json is not in v1; campaign.json `attention.top` if a writer adds it)
    if let Some(a) = c.pointer("/attention/top").and_then(Value::as_array) {
        for x in a.iter().take(100) {
            out.push(AttentionRow {
                id: s(x, "/id").unwrap_or_default(),
                code: s(x, "/code").unwrap_or_default(),
                severity: match x.get("severity").and_then(Value::as_str) {
                    Some("warning") => Severity::Warning,
                    Some("error") => Severity::Error,
                    _ => Severity::Decision,
                },
                text: s(x, "/text").unwrap_or_default(),
                options: x
                    .get("options")
                    .and_then(Value::as_array)
                    .map(|o| {
                        o.iter()
                            .take(8)
                            .filter_map(Value::as_str)
                            .map(clean)
                            .collect()
                    })
                    .unwrap_or_default(),
                derived: false,
            });
        }
    }
    let _ = raw;
    for n in &vm.nodes {
        if n.phase == "stale" || n.stale_reason.is_some() {
            out.push(AttentionRow {
                id: n.id.clone(),
                code: "stale_inputs".into(),
                severity: Severity::Warning,
                text: format!(
                    "node is STALE: {}",
                    n.stale_reason
                        .clone()
                        .unwrap_or_else(|| "an input changed".into())
                ),
                options: vec![format!("rr_campaign why {}", n.id)],
                derived: true,
            });
        }
        if n.phase == "open" {
            out.push(AttentionRow {
                id: n.id.clone(),
                code: "full_closure_not_reached".into(),
                severity: Severity::Error,
                text: format!(
                    "node OPEN: attempt ended without closure (full blockers: {}; box blockers: {})",
                    n.blockers_full.clone().unwrap_or_else(|| "-".into()),
                    n.blockers_box.clone().unwrap_or_else(|| "-".into())
                ),
                options: vec![format!("rr_campaign why {}", n.id)],
                derived: true,
            });
        }
        if let Some(k) = n.terminal_candidates.filter(|k| *k > 0) {
            out.push(AttentionRow {
                id: n.id.clone(),
                code: "terminal_candidates".into(),
                severity: Severity::Decision,
                text: format!("{k} terminal candidate item(s) wait for a decision"),
                options: vec![format!("rr_campaign terminals --node {}", n.id)],
                derived: true,
            });
        }
        if let (Some(t), Some(b)) = (n.terminals, n.terminal_bound)
            && b > 0
            && t * 10 >= b * 9
        {
            out.push(AttentionRow {
                id: n.id.clone(),
                code: "terminals_over_cap".into(),
                severity: Severity::Warning,
                text: format!("{t} terminals of the bound {b}"),
                options: vec![format!("rr_campaign terminals --node {}", n.id)],
                derived: true,
            });
        }
    }
    if matches!(vm.banner, Banner::Stopped | Banner::Stale)
        && (vm.pending_operations.unwrap_or(0) > 0 || vm.unsettled_jobs.unwrap_or(0) > 0)
    {
        out.push(AttentionRow {
            id: "campaign".into(),
            code: "interrupted".into(),
            severity: Severity::Warning,
            text: format!(
                "{} pending operation(s), {} unsettled job(s): resume with the same command",
                vm.pending_operations.unwrap_or(0),
                vm.unsettled_jobs.unwrap_or(0)
            ),
            options: vec![],
            derived: true,
        });
    }
    if let (Some(a), Some(fl)) = (
        vm.resources.host_available_bytes,
        vm.resources.host_floor_bytes,
    ) && a < fl
    {
        out.push(AttentionRow {
            id: "host".into(),
            code: "memory_below_floor".into(),
            severity: Severity::Error,
            text: format!(
                "host MemAvailable {} G below the floor {} G: no launches",
                super::format::gib(Some(a)),
                super::format::gib(Some(fl))
            ),
            options: vec![],
            derived: true,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rung_order_and_tags() {
        let mut v = vec!["CERT", "L10", "L2", "L0", "x"];
        v.sort_by_key(|r| rung_key(r));
        assert_eq!(v, ["L0", "L2", "L10", "CERT", "x"]);
        assert_eq!(event_tag("item_accepted", ""), EventTag::Ok);
        assert_eq!(event_tag("node", "T1: fall back to box B"), EventTag::Boxed);
    }

    #[test]
    fn garbage_never_panics() {
        let mut raw = RawSnapshot::empty("x");
        for v in [
            serde_json::json!({"schema":"rustred.campaign.progress.v1","resources":{"cpus_in_use":-5,"cpus_allowed":"x"},
                "items":{"by_level":{"L0":7,"Lx":{"open":1e300}},"by_phase":[1]},"eta":{"wall_seconds":{"low":"a"}},
                "jobs":{"list":[1,{"job":5}]},"files":{"nodes":{"A":"../../etc/passwd"}}}),
            serde_json::json!({"schema":"rustred.campaign.progress.v1"}),
        ] {
            raw.campaign = FileRead::Ok(v);
            raw.nodes.insert(
                "N".into(),
                FileRead::Ok(serde_json::json!({"closure_kind":5,"items":{"by_level":[]}})),
            );
            raw.events = FileRead::Ok(serde_json::json!({"events":[{"seq":"x"},7]}));
            let vm = build(
                &raw,
                BuildOptions {
                    now_s: 0.0,
                    stale_after_s: 60.0,
                },
            );
            assert_eq!(vm.nodes.len(), 1);
        }
    }
}
