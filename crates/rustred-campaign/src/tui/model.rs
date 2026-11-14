//! The dashboard's VIEW-MODEL: plain structs filled by ONE adapter function, [`build`], from the progress files.
//! Only this file knows progress field names and file names; a schema addition touches this file alone. Unknown
//! values stay `None` (shown as `?` / `-`), never 0.
//!
//! This is also the sanitising boundary (audit C1): every text field is a [`Clean`]; raw text from the files, the
//! ledger fallback or error messages becomes `Clean` here and nowhere else. Numbers that contradict each other are
//! kept, marked (`?` on screen) and listed in `problems` (audit M2), never silently.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use serde_json::Value;

use super::clean::{Clean, cl};
use super::source::{FileRead, ItemRow, JobRow, RawSnapshot};

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

/// Display word of the per-key proof level (accepted items at `identity_proved`).
pub const PER_KEY: &str = "per-key";

/// The further progress files (ledger-and-progress.md 9): key, campaign.json `files.<key>` override, default
/// name. `heartbeat.json` (`supervisor.heartbeat_utc`, rewritten at least every 10 s by a live supervisor),
/// `jobs.json` (`list`: running attempts with their live samples), `attention.json` (`items`), `terminals.json`
/// (`declared`, per node `report` / `terminals`). When a file is ABSENT (a directory of an older writer) the
/// dashboard falls back, with a visible label: staleness inferred from file times, job rows from the ledger
/// (partial), attention rows derived from the summary.
const OPTIONAL_FILES: &[(&str, &[&str])] = &[
    ("heartbeat", &["heartbeat.json"]),
    ("jobs", &["jobs.json"]),
    ("attention", &["attention.json"]),
    ("terminals", &["terminals.json"]),
];

/// The optional files a source should read: (key, path relative to `progress/`). A path named by campaign.json
/// `files.<key>` wins; otherwise the default names (absent files are fine).
pub fn optional_files(campaign: Option<&Value>) -> Vec<(String, String)> {
    let named = campaign.and_then(|c| c.get("files"));
    let mut out = Vec::new();
    for (key, defaults) in OPTIONAL_FILES {
        match named.and_then(|f| f.get(*key)).and_then(Value::as_str) {
            Some(rel) => out.push(((*key).to_string(), rel.to_string())),
            None => {
                for d in *defaults {
                    out.push(((*key).to_string(), (*d).to_string()));
                }
            }
        }
    }
    out
}

/// The first usable optional file of a key.
fn extra<'a>(raw: &'a RawSnapshot, key: &str) -> Option<&'a Value> {
    raw.extra.get(key).and_then(FileRead::value)
}

/// True when `jobs.json` is ABSENT, so the source's (ledger, partial) fallback is wanted. A present but unreadable
/// file is reported as a problem, never replaced by the fallback.
pub fn needs_job_fallback(raw: &RawSnapshot) -> bool {
    matches!(raw.extra.get("jobs"), None | Some(FileRead::Absent))
}

fn jobs_array(raw: &RawSnapshot) -> Option<&Vec<Value>> {
    extra(raw, "jobs")?.get("list").and_then(Value::as_array)
}

/// The headline state word.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Banner {
    /// The live dashboard has not received its first read yet.
    Reading,
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
            Banner::Reading => "READING",
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

/// Where the staleness judgement comes from (audit M4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum HeartbeatSource {
    /// The supervisor's heartbeat field (`heartbeat.json` `supervisor.heartbeat_utc`).
    Field,
    /// No `heartbeat.json`: inferred from the newest file time (resource sample, written_utc, mtime).
    Inferred,
    None,
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
    pub sampled_utc: Option<Clean>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct LadderRow {
    pub rung: Clean,
    /// Counts per item phase, indexed like [`PHASES`].
    pub counts: [u64; 12],
    pub jobs: Option<u64>,
    pub median_s: Option<f64>,
}

impl LadderRow {
    pub fn open(&self) -> u64 {
        open_of(&self.counts)
    }
}

fn open_of(counts: &[u64; 12]) -> u64 {
    PHASES
        .iter()
        .zip(counts)
        .filter(|(p, _)| phase_is_open(p))
        .map(|(_, c)| c)
        .sum()
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct EtaView {
    pub remaining_items: Option<u64>,
    pub wall_s: Option<(f64, f64)>,
    pub cpu_s: Option<(f64, f64)>,
    pub parallelism: Option<u64>,
    pub pooled_levels: Vec<Clean>,
    pub reason: Option<Clean>,
    pub estimator: Option<Clean>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NodeRow {
    pub id: Clean,
    pub coverage: Coverage,
    pub certified: bool,
    pub phase: Clean,
    /// `full`, or the box id of a BOXED target.
    pub target: Clean,
    pub spec_box: Option<Clean>,
    pub attempt_ended: bool,
    pub faces_closed: Option<u64>,
    pub faces_total: Option<u64>,
    pub items_open: Option<u64>,
    /// Open items per rung (non-zero only), in rung order.
    pub open_by_rung: Vec<(Clean, u64)>,
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
    /// Rules of this node in use: rules used on the closure, else the economy's count after (audit M2).
    pub rules_in_use: Option<u64>,
    /// The economy block was measured (not all zeros / not marked unmeasured).
    pub economy_measured: bool,
    pub rules_after: Option<u64>,
    pub rules_before: Option<u64>,
    pub rhs_after: Option<u64>,
    pub rhs_before: Option<u64>,
    pub superseded: Option<u64>,
    /// Per-key proved share of the accepted items (proved / accepted; None when nothing accepted).
    pub proved_share: Option<f64>,
    pub accepted_by_evidence: [u64; 3],
    pub cpu_seconds: Option<f64>,
    pub stale_reason: Option<Clean>,
    pub blockers_full: Option<Clean>,
    pub blockers_box: Option<Clean>,
    /// Why this node file is not usable (absent, invalid, unknown schema).
    pub problem: Option<Clean>,
    /// Closed against MEASURED hand-down ranges (R41 P-2): closed, labelled provisional, rechecked later.
    pub provisional: bool,
    /// Certification level asked by the spec for this node (`replay`, `per-key`, ...) and reached (R40 U-12, R41).
    pub cert_asked: Option<Clean>,
    pub cert_reached: Option<Clean>,
    /// The user's optional PREFERRED terminals (R41 P-4): total in the wish list, used, not needed.
    pub preferred: Option<Preferred>,
    /// Read-only REFERENCE numbers (an existing ledger): comparison only, never an error.
    pub reference: Option<Reference>,
    /// An attention row names this node (sort order, `!` marker).
    pub attention: bool,
    /// Displayed fields whose numbers contradict each other (`?` marker): `rules`, `used`, `open`.
    pub suspect: BTreeSet<&'static str>,
}

impl NodeRow {
    /// Certified at the level its spec asks (the levels when stated, else the `certified` flag).
    pub fn at_level(&self) -> bool {
        match (&self.cert_asked, &self.cert_reached) {
            (Some(a), Some(r)) => a == r,
            (Some(_), None) => false,
            _ => self.certified,
        }
    }

    pub fn accepted(&self) -> u64 {
        self.accepted_by_evidence.iter().sum()
    }

    /// Closed (FULL or BOXED) in a final phase, or certified.
    pub fn closed(&self) -> bool {
        matches!(self.coverage, Coverage::Full | Coverage::Boxed)
    }
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
    pub name: Option<Clean>,
    pub terminals: Option<u64>,
    pub rules: Option<u64>,
    pub rhs_terms: Option<u64>,
    pub status: Option<Clean>,
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
    pub utc: Clean,
    pub kind: Clean,
    pub text: Clean,
    pub tag: EventTag,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum Severity {
    Error,
    Decision,
    Warning,
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
    pub id: Clean,
    pub code: Clean,
    pub severity: Severity,
    pub text: Clean,
    pub options: Vec<Clean>,
    /// True when computed by the dashboard (no attention file in the progress files).
    pub derived: bool,
}

/// One running job, sanitised.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct JobView {
    pub job: Clean,
    pub node: Clean,
    pub item: Clean,
    pub rung: Clean,
    pub cpus: Option<u64>,
    pub rss_bytes: Option<u64>,
    pub cap_bytes: Option<u64>,
    pub elapsed_s: Option<f64>,
    pub budget_s: Option<f64>,
    pub heartbeat_age_s: Option<f64>,
    pub phase: Option<Clean>,
    pub pid: Option<u64>,
    pub run_dir: Option<Clean>,
}

/// Where the running-job rows come from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub enum JobsOrigin {
    /// No rows known.
    #[default]
    None,
    /// The progress files (jobs file or campaign.json `jobs.list`).
    Progress,
    /// The source's ledger reduction: job, node, item, rung, reservation, pid, run dir only.
    LedgerPartial,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct JobsView {
    pub total: Option<u64>,
    pub running: Option<u64>,
    pub launching: Option<u64>,
    pub queued: Option<u64>,
    pub by_state: BTreeMap<Clean, u64>,
    pub settled_by_class: BTreeMap<Clean, u64>,
    pub cap: Option<u64>,
    pub list: Option<Vec<JobView>>,
    pub list_origin: JobsOrigin,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct RulesView {
    pub accepted: Option<u64>,
    pub rejected: Option<u64>,
    pub rejected_by_reason: BTreeMap<Clean, u64>,
    pub upgrades: Option<u64>,
    pub retired: Option<u64>,
    /// geometry_accepted, identity_tested, identity_proved
    pub by_evidence: [Option<u64>; 3],
    pub rules_before: Option<u64>,
    pub rules_after: Option<u64>,
    pub rhs_before: Option<u64>,
    pub rhs_after: Option<u64>,
}

/// Certification headline (audit M3, rulings R40 U-12 / R41): nodes certified at the level THEIR spec asks, and the
/// accepted rules by level.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct CertView {
    pub nodes_at_level: u64,
    pub nodes: u64,
    /// Accepted rules by the level that certifies them (`replay`, [`PER_KEY`], ...), and those still pending.
    pub rules_by_level: BTreeMap<Clean, u64>,
    pub rules_pending: u64,
    /// True when no node file states its levels (the `certified` flag is used instead).
    pub levels_unstated: bool,
    /// From campaign.json `certification.rules` (true) or derived from the node files (false).
    pub from_file: bool,
    /// Per-key proved / accepted items of the whole campaign (a detail, never the headline).
    pub per_key_proved: Option<u64>,
    pub per_key_accepted: Option<u64>,
}

impl CertView {
    /// proved / accepted, None when nothing is accepted (audit m2: never a share without its denominator).
    pub fn per_key_share(&self) -> Option<f64> {
        match (self.per_key_proved, self.per_key_accepted) {
            (Some(p), Some(a)) if a > 0 => Some(p as f64 / a as f64),
            _ => None,
        }
    }
}

/// Everything the screens show.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ViewModel {
    pub origin: Clean,
    pub banner: Banner,
    /// Raw campaign phase word.
    pub phase: Option<Clean>,
    pub campaign_id: Option<Clean>,
    pub title: Option<Clean>,
    pub now_s: f64,
    pub now_utc: Clean,
    pub seq: Option<u64>,
    pub last_event_utc: Option<Clean>,
    pub last_event_age_s: Option<f64>,
    pub written_utc: Option<Clean>,
    pub heartbeat_age_s: Option<f64>,
    pub heartbeat_utc: Option<Clean>,
    pub heartbeat_source: HeartbeatSource,
    pub stale_after_s: f64,
    /// Age of the newest snapshot the live dashboard has (set by the live loop; None = read just now).
    pub data_age_s: Option<f64>,
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
    pub nodes_by_phase: BTreeMap<Clean, u64>,
    pub items_total: Option<u64>,
    pub items_by_phase: [Option<u64>; 12],
    pub items_open: Option<u64>,
    pub ladder: Vec<LadderRow>,
    pub rules: RulesView,
    pub terminals_declared: Option<u64>,
    pub terminal_candidates: Option<u64>,
    pub terminal_bound: Option<u64>,
    pub cert: CertView,
    pub defect_keys: Option<u64>,
    pub nodes_checked: Option<u64>,
    pub nodes_with_defect: Option<u64>,
    pub faces_closed: Option<u64>,
    pub faces_total: Option<u64>,
    pub cost_cpu_s: Option<f64>,
    pub budget_cpu_hours: Option<f64>,
    /// Campaign mode (`generate`, `optimise`, ...); `optimise` shows economy before / after.
    pub mode: Option<Clean>,
    /// Certification level asked by the spec for the campaign (when the campaign file states it).
    pub cert_asked: Option<Clean>,
    pub preferred: Option<Preferred>,
    pub eta: EtaView,
    /// Sorted: attention first, then running, OPEN, BOXED, FULL (audit m7).
    pub nodes: Vec<NodeRow>,
    pub jobs: JobsView,
    /// Newest first.
    pub events: Vec<EventRow>,
    /// Sorted by severity (error, decision, warning).
    pub attention: Vec<AttentionRow>,
    /// Problems with the inputs (unreadable files, unknown schemas, last good copies in use, contradictions).
    pub problems: Vec<Clean>,
    /// Campaign-level displayed numbers that contradict each other (`?` marker): `cert`, `nodes`, `items`, `defect`.
    pub suspect: BTreeSet<&'static str>,
}

impl ViewModel {
    /// Live-looking values (CPUs, jobs, RAM, ETA) are old: STALE campaign or a lagging reader (audit M4).
    pub fn live_values_old(&self) -> bool {
        self.banner == Banner::Stale || self.data_age_s.is_some_and(|a| a > 10.0)
    }

    /// Age to show beside live values when they are old.
    pub fn live_age_s(&self) -> Option<f64> {
        if self.banner == Banner::Stale {
            self.heartbeat_age_s
        } else {
            self.data_age_s
        }
    }

    /// Items in a final phase (accepted, escalated, terminal, retired).
    pub fn items_done(&self) -> Option<u64> {
        Some(self.items_total?.saturating_sub(self.items_open?))
    }
}

fn u(v: &Value, p: &str) -> Option<u64> {
    num(v.pointer(p)?)
}

fn num(x: &Value) -> Option<u64> {
    x.as_u64().or_else(|| {
        x.as_f64()
            .filter(|f| f.is_finite() && *f >= 0.0)
            .map(|f| f as u64)
    })
}

fn f(v: &Value, p: &str) -> Option<f64> {
    v.pointer(p)?.as_f64().filter(|x| x.is_finite())
}

fn s(v: &Value, p: &str) -> Option<Clean> {
    v.pointer(p)?.as_str().map(Clean::new)
}

fn nonzero(x: Option<u64>) -> Option<u64> {
    x.filter(|v| *v > 0)
}

fn counts_map(v: Option<&Value>) -> BTreeMap<Clean, u64> {
    let mut m = BTreeMap::new();
    if let Some(o) = v.and_then(Value::as_object) {
        for (k, x) in o.iter().take(256) {
            if let Some(n) = x.as_u64() {
                m.insert(cl(k), n);
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

fn iso_of(t: f64) -> Clean {
    cl(crate::clock::UtcMillis((t * 1000.0) as i64).to_iso())
}

/// Generic natural sort key of a rung name (audit m6, rule L8): names with a number sort by the number, then by the
/// text around it (`L0 < L2 < L10`, `P2 < P10`); names without a number come after, alphabetically.
pub fn rung_key(r: &str) -> (u8, u64, String, String) {
    let start = r.find(|c: char| c.is_ascii_digit());
    match start {
        Some(i) => {
            let digits: String = r[i..].chars().take_while(char::is_ascii_digit).collect();
            let n = digits.parse::<u64>().unwrap_or(u64::MAX);
            (0, n, r[..i].to_string(), r[i + digits.len()..].to_string())
        }
        None => (1, 0, r.to_string(), String::new()),
    }
}

fn blockers(v: Option<&Value>) -> Option<Clean> {
    let b = v.filter(|x| x.is_object())?;
    let n = |k: &str| b.get(k).and_then(Value::as_u64).unwrap_or(0);
    Some(cl(format!(
        "{} faces, {} loci, {} keys",
        n("faces"),
        n("loci"),
        n("keys")
    )))
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
    /// Attention "terminals near the bound" from this share of the bound on (display heuristic; 0.9).
    pub terminal_warn_share: f64,
}

impl BuildOptions {
    pub fn at(now_s: f64) -> Self {
        BuildOptions {
            now_s,
            stale_after_s: 60.0,
            terminal_warn_share: 0.9,
        }
    }
}

/// Stamps more than this far in the future are another host's clock: ignored (audit m9).
const FUTURE_TOLERANCE_S: f64 = 5.0;

fn blank_node(id: &str) -> NodeRow {
    NodeRow {
        id: cl(id),
        coverage: Coverage::Unknown,
        certified: false,
        phase: cl("?"),
        target: cl("?"),
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
        rules_in_use: None,
        economy_measured: false,
        rules_after: None,
        rules_before: None,
        rhs_after: None,
        rhs_before: None,
        superseded: None,
        proved_share: None,
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
        attention: false,
        suspect: BTreeSet::new(),
    }
}

fn node_row(id: &str, r: &FileRead, problems: &mut Vec<Clean>) -> NodeRow {
    let mut row = blank_node(id);
    let v = match r {
        FileRead::Ok(v) => v,
        FileRead::Absent => {
            row.problem = Some(cl("node file absent"));
            return row;
        }
        FileRead::Invalid(w) | FileRead::UnknownSchema(w) => {
            row.problem = Some(cl(w));
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
    row.phase = s(v, "/phase").unwrap_or_else(|| cl("?"));
    row.target = match v.pointer("/target/target").and_then(Value::as_str) {
        Some("full") => cl("full"),
        Some("boxed") => s(v, "/target/box_id").unwrap_or_else(|| cl("box")),
        Some(o) => cl(o),
        None => cl("?"),
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
        if o.len() > 64 {
            problems.push(cl(format!(
                "node {}: {} rungs, only 64 shown",
                row.id,
                o.len()
            )));
        }
        let mut rows: Vec<(Clean, u64)> = o
            .iter()
            .take(64)
            .map(|(r, c)| (cl(r), open_of(&phase_array(Some(c)))))
            .filter(|(_, n)| *n > 0)
            .collect();
        rows.sort_by_key(|(r, _)| rung_key(r.as_str()));
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
    // an economy block of zeros (e.g. an imported table) was not measured: its 0 is unknown, not zero (audit M2)
    row.economy_measured = v.get("economy").is_some_and(Value::is_object)
        && (row.rules_after.unwrap_or(0) > 0 || row.rules_before.unwrap_or(0) > 0);
    if !row.economy_measured {
        row.rules_after = None;
        row.rules_before = None;
        row.rhs_after = None;
        row.rhs_before = None;
        row.superseded = None;
    }
    row.rules_in_use = row.rules_used.or(row.rules_after);
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
    let acc = row.accepted();
    row.proved_share = (acc > 0).then(|| row.accepted_by_evidence[2] as f64 / acc as f64);
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
    check_node(&mut row, problems);
    row
}

/// Invariants of one node (audit M2): contradicting numbers are marked and listed.
fn check_node(n: &mut NodeRow, problems: &mut Vec<Clean>) {
    let mut found: Vec<(&'static str, String)> = Vec::new();
    let mut bad = |key: &'static str, why: String, _: &NodeRow| found.push((key, why));
    if let (Some(us), Some(to)) = (n.rules_used, n.rules_total) {
        if us > to {
            bad("rules", format!("rules used {us} > rules total {to}"), n);
        } else if let Some(sh) = n.used_share
            && to > 0
            && (sh - us as f64 / to as f64).abs() > 0.005
        {
            bad(
                "used",
                format!(
                    "used share {:.1}% but {us} of {to} = {:.1}%",
                    sh * 100.0,
                    us as f64 * 100.0 / to as f64
                ),
                n,
            );
        }
    }
    if let Some(o) = n.items_open {
        let sum = open_of(&n.items_by_phase);
        if n.items_by_phase.iter().any(|k| *k > 0) && sum != o {
            bad(
                "open",
                format!("items open {o} but open phases sum to {sum}"),
                n,
            );
        }
    }
    for (key, why) in found {
        n.suspect.insert(key);
        problems.push(cl(format!("inconsistent: node {}: {why}", n.id)));
    }
}

fn job_view(j: &JobRow) -> JobView {
    JobView {
        job: cl(&j.job),
        node: cl(&j.node),
        item: cl(&j.item),
        rung: cl(&j.rung),
        cpus: j.cpus,
        rss_bytes: j.rss_bytes,
        cap_bytes: j.cap_bytes,
        elapsed_s: j.elapsed_s,
        budget_s: j.budget_s,
        heartbeat_age_s: j.heartbeat_age_s,
        phase: j.phase.as_deref().map(Clean::new),
        pid: j.pid,
        run_dir: j.run_dir.as_deref().map(Clean::new),
    }
}

/// Sanitised running jobs of a source's ledger fallback.
pub fn ledger_jobs(rows: &[JobRow]) -> Vec<JobView> {
    rows.iter().take(1024).map(job_view).collect()
}

/// One work item of a node, sanitised (drill-down).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ItemView {
    pub item: Clean,
    pub label: Clean,
    pub kind: Clean,
    pub rung: Clean,
    pub state: Clean,
    pub why: Clean,
    pub cpu_seconds: f64,
    pub jobs: usize,
    pub updated_seq: u64,
}

/// The sanitising boundary of the drill-down: item rows, item detail and error texts.
pub fn item_views(r: Result<Vec<ItemRow>, String>) -> Result<Vec<ItemView>, Clean> {
    match r {
        Ok(v) => Ok(v
            .into_iter()
            .map(|i| ItemView {
                item: cl(&i.item),
                label: cl(&i.label),
                kind: cl(&i.kind),
                rung: cl(&i.rung),
                state: cl(&i.state),
                why: cl(&i.why),
                cpu_seconds: i.cpu_seconds,
                jobs: i.jobs,
                updated_seq: i.updated_seq,
            })
            .collect()),
        Err(e) => Err(cl(e)),
    }
}

/// Item detail with every string and key sanitised.
pub fn item_detail_view(r: Result<Value, String>) -> Result<Value, Clean> {
    r.map(super::clean::clean_value).map_err(cl)
}

/// The view-model shown before the first read of the live dashboard has arrived.
pub fn waiting(origin: &str, waited_s: f64, o: BuildOptions) -> ViewModel {
    let mut vm = build(&RawSnapshot::empty(origin), o);
    vm.banner = Banner::Reading;
    vm.problems = vec![cl(format!(
        "reading the progress files of {} ({:.0} s so far; the screen stays responsive)",
        vm.origin, waited_s
    ))];
    vm.data_age_s = Some(waited_s);
    vm
}

/// THE adapter: progress files -> view-model.
pub fn build(raw: &RawSnapshot, o: BuildOptions) -> ViewModel {
    let mut problems: Vec<Clean> = raw.notes.iter().map(cl).collect();
    let null = Value::Null;
    let (c, banner0) = match &raw.campaign {
        FileRead::Ok(v) => (v, None),
        FileRead::Absent => {
            problems.push(cl(format!(
                "progress/campaign.json absent (campaign not started yet, or wrong directory): {}",
                raw.origin
            )));
            (&null, Some(Banner::NoData))
        }
        FileRead::Invalid(w) | FileRead::UnknownSchema(w) => {
            problems.push(cl(format!("campaign.json: {w}")));
            (&null, Some(Banner::Unreadable))
        }
    };
    for (key, r) in &raw.extra {
        if let FileRead::Invalid(w) | FileRead::UnknownSchema(w) = r {
            problems.push(cl(format!("{key} file: {w}")));
        }
    }
    let hb_file = extra(raw, "heartbeat");
    let hb_present = !matches!(raw.extra.get("heartbeat"), None | Some(FileRead::Absent));
    let sup = hb_file
        .and_then(|h| h.get("supervisor"))
        .filter(|v| v.is_object());
    let phase = s(c, "/campaign/phase");
    let written = s(c, "/written_utc");
    let res_v = c.get("resources").unwrap_or(&null);
    let mut resources = ResourcesView {
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
    // zeros of a block that was never sampled are unknown (audit m3): no sample time and no allowed CPUs, or a
    // zero RSS while jobs are live (the writer writes 0 for "not sampled")
    if resources.cpus_allowed.is_none() && resources.sampled_utc.is_none() {
        resources.cpus_in_use = nonzero(resources.cpus_in_use);
        resources.rss_bytes = nonzero(resources.rss_bytes);
        resources.reserved_bytes = nonzero(resources.reserved_bytes);
        resources.live_jobs = nonzero(resources.live_jobs);
    }
    if resources.rss_bytes == Some(0) && resources.live_jobs.unwrap_or(0) > 0 {
        resources.rss_bytes = None;
    }
    // heartbeat (audit M4): heartbeat.json's field; inferred from file times ONLY when that file is absent
    let hb_field = sup.and_then(|v| s(v, "/heartbeat_utc"));
    // (stamp, written by a clock of its own): a written stamp in the future is another host's clock and is
    // reported; a file time in the future (the viewer's own file system, or a test clock) is ignored silently
    let mut stamps: Vec<(f64, bool)> = Vec::new();
    let heartbeat_source = if let Some(t) = hb_field.as_deref_str().and_then(iso_s) {
        stamps.push((t, true));
        HeartbeatSource::Field
    } else if hb_present {
        // the file is there but names no live supervisor heartbeat (supervisor gone): no age, never inferred
        HeartbeatSource::Field
    } else {
        for t in [
            resources.sampled_utc.as_deref_str().and_then(iso_s),
            written.as_deref_str().and_then(iso_s),
        ]
        .into_iter()
        .flatten()
        {
            stamps.push((t, true));
        }
        if let Some(t) = raw.campaign_mtime_s {
            stamps.push((t, false));
        }
        if stamps.is_empty() {
            HeartbeatSource::None
        } else {
            HeartbeatSource::Inferred
        }
    };
    let mut hb: Option<f64> = None;
    for (t, written_stamp) in stamps {
        if t > o.now_s + FUTURE_TOLERANCE_S {
            if written_stamp {
                problems.push(cl(format!(
                    "clock skew: a heartbeat stamp is {} in the future (ignored)",
                    super::format::dur(Some(t - o.now_s))
                )));
            }
            continue;
        }
        hb = Some(hb.map_or(t, |h: f64| h.max(t)));
    }
    let heartbeat_age_s = hb.map(|h| (o.now_s - h).max(0.0));
    let live = matches!(
        phase.as_deref_str(),
        Some("starting" | "running" | "draining")
    );
    let stale = live && heartbeat_age_s.is_some_and(|a| a > o.stale_after_s);
    let banner = banner0.unwrap_or(if stale {
        Banner::Stale
    } else {
        match phase.as_deref_str() {
            Some("starting") => Banner::Starting,
            Some("running") => Banner::Running,
            Some("draining") => Banner::Draining,
            Some("stopped") => Banner::Stopped,
            Some("finished") => Banner::Finished,
            _ => Banner::Unknown,
        }
    });
    let started = sup
        .and_then(|v| s(v, "/started_utc"))
        .or_else(|| s(c, "/supervisor/started_utc"));
    let (elapsed_s, elapsed_label) = match started.as_deref_str().and_then(iso_s) {
        Some(t) => (Some((o.now_s - t).max(0.0)), "up"),
        None => (
            s(c, "/campaign/created_utc")
                .as_deref_str()
                .and_then(iso_s)
                .map(|t| (o.now_s - t).max(0.0)),
            "age",
        ),
    };
    // ladder: rung names and counts from the data (rule L8), cost looked up by key (no JSON pointer: names may
    // contain '/' or '~', audit m6)
    let by_level_cost = c.pointer("/cost/by_level");
    let cost_of = |r: &str, k: &str| by_level_cost.and_then(|b| b.get(r)).and_then(|x| x.get(k));
    let mut ladder: Vec<LadderRow> = Vec::new();
    let mut rung_names: Vec<String> = Vec::new();
    if let Some(m) = c.pointer("/items/by_level").and_then(Value::as_object) {
        rung_names.extend(m.keys().cloned());
    }
    if let Some(m) = by_level_cost.and_then(Value::as_object) {
        for r in m.keys() {
            if !rung_names.contains(r) {
                rung_names.push(r.clone());
            }
        }
    }
    if rung_names.len() > 64 {
        problems.push(cl(format!(
            "{} rungs in campaign.json, only 64 shown",
            rung_names.len()
        )));
    }
    for r in rung_names.iter().take(64) {
        ladder.push(LadderRow {
            rung: cl(r),
            counts: phase_array(c.pointer("/items/by_level").and_then(|m| m.get(r))),
            jobs: cost_of(r, "jobs").and_then(num),
            median_s: cost_of(r, "p50_s")
                .and_then(Value::as_f64)
                .filter(|x| x.is_finite()),
        });
    }
    ladder.sort_by_key(|l| rung_key(l.rung.as_str()));
    let ibp = c.pointer("/items/by_phase");
    let mut items_by_phase = [None; 12];
    if ibp.is_some_and(Value::is_object) {
        for (i, p) in PHASES.iter().enumerate() {
            items_by_phase[i] = ibp.and_then(|o| o.get(*p)).and_then(Value::as_u64);
        }
    }
    let items_open = ibp.is_some_and(Value::is_object).then(|| {
        PHASES
            .iter()
            .zip(items_by_phase)
            .filter(|(p, _)| phase_is_open(p))
            .map(|(_, n)| n.unwrap_or(0))
            .sum()
    });
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
                    .map(cl)
                    .collect()
            })
            .unwrap_or_default(),
        reason: s(c, "/eta/reason"),
        estimator: s(c, "/eta/estimator"),
    };
    // nodes
    let mut nodes: Vec<NodeRow> = raw
        .nodes
        .iter()
        .map(|(id, r)| node_row(id, r, &mut problems))
        .collect();
    for n in &nodes {
        if let Some(p) = &n.problem {
            problems.push(cl(format!("node {}: {p}", n.id)));
        }
    }
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
    // jobs: the running-job list of jobs.json (the ledger fallback of the source only when that file is absent)
    let by_state = counts_map(c.pointer("/jobs/by_state"));
    let list_v = jobs_array(raw);
    if list_v.is_some_and(|a| a.len() > 1024) {
        problems.push(cl(
            "more than 1024 running jobs listed; the first 1024 shown",
        ));
    }
    let list_from_file: Option<Vec<JobView>> = list_v.map(|a| {
        a.iter()
            .take(1024)
            .map(|j| JobView {
                job: s(j, "/job").unwrap_or_default(),
                node: s(j, "/node").unwrap_or_default(),
                item: s(j, "/item").or_else(|| s(j, "/label")).unwrap_or_default(),
                rung: s(j, "/rung").unwrap_or_default(),
                cpus: u(j, "/cpus").or_else(|| u(j, "/cpu").map(|_| 1)),
                rss_bytes: u(j, "/rss_bytes"),
                cap_bytes: u(j, "/budget_mem_bytes").or_else(|| u(j, "/reserved_bytes")),
                elapsed_s: f(j, "/seconds").or_else(|| f(j, "/elapsed_s")),
                budget_s: f(j, "/budget_seconds"),
                heartbeat_age_s: s(j, "/heartbeat_utc")
                    .as_deref_str()
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
            .get(&cl("running"))
            .copied()
            .or_else(|| u(c, "/jobs/running")),
        launching: by_state.get(&cl("launching")).copied(),
        queued: by_state.get(&cl("queued")).copied(),
        settled_by_class: counts_map(c.pointer("/jobs/settled_by_class")),
        by_state,
        cap: u(c, "/jobs/cap"),
        list_origin: if list_from_file.is_some() {
            JobsOrigin::Progress
        } else {
            JobsOrigin::None
        },
        list: list_from_file,
    };
    // events, newest first
    let events_v = match &raw.events {
        FileRead::Ok(v) => v.get("events").and_then(Value::as_array).cloned(),
        FileRead::Absent => None,
        FileRead::Invalid(w) | FileRead::UnknownSchema(w) => {
            problems.push(cl(format!("events.json: {w}")));
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
            let kind = s(e, "/kind").unwrap_or_else(|| cl("?"));
            let text = s(e, "/text").unwrap_or_default();
            EventRow {
                seq: u(e, "/seq").unwrap_or(0),
                utc: s(e, "/utc").unwrap_or_default(),
                tag: event_tag(kind.as_str(), text.as_str()),
                kind,
                text,
            }
        })
        .collect();
    events.sort_by_key(|e| std::cmp::Reverse(e.seq));
    let last_event_utc = s(c, "/ledger/last_utc");
    let mut vm = ViewModel {
        origin: cl(&raw.origin),
        banner,
        phase,
        campaign_id: s(c, "/campaign_id"),
        title: s(c, "/title"),
        now_s: o.now_s,
        now_utc: iso_of(o.now_s),
        seq: u(c, "/ledger/seq"),
        last_event_age_s: last_event_utc
            .as_deref_str()
            .and_then(iso_s)
            .map(|t| (o.now_s - t).max(0.0)),
        last_event_utc,
        written_utc: written,
        heartbeat_age_s,
        heartbeat_utc: hb.map(iso_of),
        heartbeat_source,
        stale_after_s: o.stale_after_s,
        data_age_s: None,
        elapsed_s,
        elapsed_label,
        sessions: u(c, "/campaign/sessions"),
        crash_recoveries: u(c, "/campaign/crash_recoveries"),
        lock_takeovers: u(c, "/campaign/lock_takeovers"),
        torn_tail_recoveries: u(c, "/campaign/torn_tail_recoveries"),
        pending_operations: u(c, "/campaign/pending_operations"),
        unsettled_jobs: u(c, "/campaign/unsettled_jobs"),
        grace_left_s: sup
            .and_then(|v| f(v, "/grace_left_s"))
            .or_else(|| f(c, "/supervisor/grace_left_s")),
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
        cert: CertView::default(),
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
        nodes: Vec::new(),
        jobs,
        events,
        attention: Vec::new(),
        problems: Vec::new(),
        suspect: BTreeSet::new(),
    };
    // terminals.json: the declared total (candidates are counted in campaign.json)
    if let Some(n) = extra(raw, "terminals").and_then(|t| u(t, "/declared")) {
        vm.terminals_declared = Some(n);
    }
    vm.cert = certification(c, &nodes, &vm.rules);
    vm.attention = attention(c, raw, &vm, &nodes, o);
    // item rows of attention.json name their node
    let attention_nodes: BTreeSet<Clean> = extra(raw, "attention")
        .and_then(|a| a.get("items"))
        .and_then(Value::as_array)
        .map(|a| a.iter().take(100).filter_map(|x| s(x, "/node")).collect())
        .unwrap_or_default();
    for n in &mut nodes {
        n.attention = vm.attention.iter().any(|a| a.id == n.id) || attention_nodes.contains(&n.id);
    }
    nodes.sort_by(|a, b| {
        node_rank(a)
            .cmp(&node_rank(b))
            .then_with(|| a.id.cmp(&b.id))
    });
    vm.nodes = nodes;
    for (key, why) in check_campaign(&vm, raw) {
        vm.suspect.insert(key);
        problems.push(cl(format!("inconsistent: {why}")));
    }
    vm.problems = problems;
    vm
}

/// Sort order of the nodes table (audit m7): attention, running, OPEN, BOXED, FULL, unknown.
fn node_rank(n: &NodeRow) -> u8 {
    if n.attention {
        0
    } else if matches!(n.phase.as_str(), "running" | "closure_check" | "registered") {
        1
    } else {
        match n.coverage {
            Coverage::Open => 2,
            Coverage::Boxed => 3,
            Coverage::Full => 4,
            Coverage::Unknown => 5,
        }
    }
}

fn certification(c: &Value, nodes: &[NodeRow], rules: &RulesView) -> CertView {
    let mut v = CertView {
        nodes: nodes.len() as u64,
        nodes_at_level: nodes.iter().filter(|n| n.at_level()).count() as u64,
        levels_unstated: !nodes
            .iter()
            .any(|n| n.cert_asked.is_some() || n.cert_reached.is_some()),
        per_key_proved: rules.by_evidence[2].or_else(|| u(c, "/certified/identity_proved")),
        per_key_accepted: u(c, "/certified/accepted").or_else(|| {
            rules
                .by_evidence
                .iter()
                .try_fold(0u64, |a, x| x.map(|x| a + x))
        }),
        ..CertView::default()
    };
    if let Some(r) = c.pointer("/certification/rules").and_then(Value::as_object) {
        v.from_file = true;
        for (k, x) in r.iter().take(16) {
            if let Some(n) = num(x) {
                if k == "pending" {
                    v.rules_pending = n;
                } else {
                    v.rules_by_level.insert(cl(k.replace('_', "-")), n);
                }
            }
        }
        return v;
    }
    // derived: a node at its asked level certifies all its accepted rules at that level; elsewhere the per-key
    // proved rules count per key and the rest is pending
    for n in nodes {
        let acc = n.accepted();
        if acc == 0 {
            continue;
        }
        if n.at_level() {
            let level = n
                .cert_reached
                .clone()
                .unwrap_or_else(|| cl("certified node"));
            *v.rules_by_level.entry(level).or_insert(0) += acc;
        } else {
            let p = n.accepted_by_evidence[2];
            if p > 0 {
                *v.rules_by_level.entry(cl(PER_KEY)).or_insert(0) += p;
            }
            v.rules_pending += acc - p;
        }
    }
    v
}

/// Campaign-level invariants (audit M2): the displayed numbers must be recomputable from each other.
fn check_campaign(vm: &ViewModel, raw: &RawSnapshot) -> Vec<(&'static str, String)> {
    let mut found: Vec<(&'static str, String)> = Vec::new();
    let mut bad = |_: &ViewModel, key: &'static str, why: String| found.push((key, why));
    let c = raw.campaign.value();
    if let Some(sh) = c.and_then(|c| f(c, "/certified/share")) {
        match vm.cert.per_key_share() {
            Some(x) if (x - sh).abs() > 0.0005 => bad(
                vm,
                "cert",
                format!(
                    "certified.share {:.1}% but {} of {} proved = {:.1}%",
                    sh * 100.0,
                    vm.cert.per_key_proved.unwrap_or(0),
                    vm.cert.per_key_accepted.unwrap_or(0),
                    x * 100.0
                ),
            ),
            None if sh > 0.0 => bad(
                vm,
                "cert",
                format!("certified.share {:.1}% with nothing accepted", sh * 100.0),
            ),
            _ => {}
        }
    }
    if let Some(t) = vm.nodes_total {
        let files = raw.nodes.len() as u64;
        if files > 0 && files != t {
            bad(
                vm,
                "nodes",
                format!("nodes.total {t} but {files} node files"),
            );
        }
        if let (Some(a), Some(b), Some(o)) = (vm.nodes_full, vm.nodes_boxed, vm.nodes_open)
            && a + b + o != t
        {
            bad(
                vm,
                "nodes",
                format!("FULL {a} + BOXED {b} + OPEN {o} != nodes.total {t}"),
            );
        }
        if let Some(k) = vm.nodes_certified
            && k > t
        {
            bad(vm, "nodes", format!("{k} certified nodes of {t}"));
        }
        if let Some(k) = vm.nodes_checked
            && k > t
        {
            bad(vm, "defect", format!("{k} nodes checked of {t} nodes"));
        }
    }
    if let (Some(d), Some(k)) = (vm.nodes_with_defect, vm.nodes_checked)
        && d > k
    {
        bad(
            vm,
            "defect",
            format!("{d} nodes with defect of {k} checked"),
        );
    }
    if let Some(t) = vm.items_total
        && vm.items_by_phase.iter().any(Option::is_some)
    {
        let sum: u64 = vm.items_by_phase.iter().map(|x| x.unwrap_or(0)).sum();
        if sum != t {
            bad(
                vm,
                "items",
                format!("items.total {t} but the phases sum to {sum}"),
            );
        }
    }
    if let Some(d) = vm.defect_keys
        && !vm.nodes.is_empty()
        && vm.nodes.iter().all(|n| n.defect_keys.is_some())
    {
        let sum: u64 = vm.nodes.iter().filter_map(|n| n.defect_keys).sum();
        if sum != d {
            bad(
                vm,
                "defect",
                format!("closure.defect_keys {d} but the node files sum to {sum}"),
            );
        }
    }
    found
}

fn attention(
    c: &Value,
    raw: &RawSnapshot,
    vm: &ViewModel,
    nodes: &[NodeRow],
    o: BuildOptions,
) -> Vec<AttentionRow> {
    let mut out = Vec::new();
    // attention.json `items` (campaign.json `attention.top` while that file is absent); rows DERIVED from the summary
    // only when neither is there (shown as derived)
    let given = extra(raw, "attention")
        .and_then(|a| a.get("items"))
        .or_else(|| c.pointer("/attention/top"))
        .and_then(Value::as_array);
    if let Some(a) = given {
        for x in a.iter().take(100) {
            out.push(AttentionRow {
                id: s(x, "/id").or_else(|| s(x, "/node")).unwrap_or_default(),
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
                    .map(|o| o.iter().take(8).filter_map(Value::as_str).map(cl).collect())
                    .unwrap_or_default(),
                derived: false,
            });
        }
        out.sort_by_key(|a| a.severity);
        return out;
    }
    for n in nodes {
        if n.phase == "stale" || n.stale_reason.is_some() {
            out.push(AttentionRow {
                id: n.id.clone(),
                code: cl("stale_inputs"),
                severity: Severity::Warning,
                text: cl(format!(
                    "node is STALE: {}",
                    n.stale_reason
                        .clone()
                        .unwrap_or_else(|| cl("an input changed"))
                )),
                options: vec![cl(format!("rr_campaign why {}", n.id))],
                derived: true,
            });
        }
        if n.phase == "open" {
            out.push(AttentionRow {
                id: n.id.clone(),
                code: cl("full_closure_not_reached"),
                severity: Severity::Error,
                text: cl(format!(
                    "node OPEN: attempt ended without closure (full blockers: {}; box blockers: {})",
                    n.blockers_full.clone().unwrap_or_else(|| cl("-")),
                    n.blockers_box.clone().unwrap_or_else(|| cl("-"))
                )),
                options: vec![cl(format!("rr_campaign why {}", n.id))],
                derived: true,
            });
        }
        if let Some(k) = n.terminal_candidates.filter(|k| *k > 0) {
            out.push(AttentionRow {
                id: n.id.clone(),
                code: cl("terminal_candidates"),
                severity: Severity::Decision,
                text: cl(format!(
                    "{k} terminal candidate item(s) wait for a decision"
                )),
                options: vec![cl(format!("rr_campaign terminals --node {}", n.id))],
                derived: true,
            });
        }
        if let (Some(t), Some(b)) = (n.terminals, n.terminal_bound)
            && b > 0
            && t as f64 >= b as f64 * o.terminal_warn_share
        {
            out.push(AttentionRow {
                id: n.id.clone(),
                code: cl("terminals_near_bound"),
                severity: Severity::Warning,
                text: cl(format!("{t} terminals of the bound {b}")),
                options: vec![cl(format!("rr_campaign terminals --node {}", n.id))],
                derived: true,
            });
        }
    }
    if matches!(vm.banner, Banner::Stopped | Banner::Stale)
        && (vm.pending_operations.unwrap_or(0) > 0 || vm.unsettled_jobs.unwrap_or(0) > 0)
    {
        out.push(AttentionRow {
            id: cl("campaign"),
            code: cl("interrupted"),
            severity: Severity::Warning,
            text: cl(format!(
                "{} pending operation(s), {} unsettled job(s): resume with the same command",
                vm.pending_operations.unwrap_or(0),
                vm.unsettled_jobs.unwrap_or(0)
            )),
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
            id: cl("host"),
            code: cl("memory_below_floor"),
            severity: Severity::Error,
            text: cl(format!(
                "host MemAvailable {} GiB below the floor {} GiB: no launches",
                super::format::gib(Some(a)),
                super::format::gib(Some(fl))
            )),
            options: vec![],
            derived: true,
        });
    }
    out.sort_by_key(|a| a.severity);
    out
}

/// `Option<Clean>` -> `Option<&str>`.
trait AsDerefStr {
    fn as_deref_str(&self) -> Option<&str>;
}

impl AsDerefStr for Option<Clean> {
    fn as_deref_str(&self) -> Option<&str> {
        self.as_ref().map(Clean::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rung_order_is_generic_and_tags() {
        let mut v = vec!["CERT", "L10", "L2", "L0", "x", "P10", "P2"];
        v.sort_by_key(|r| rung_key(r));
        assert_eq!(v, ["L0", "L2", "P2", "L10", "P10", "CERT", "x"]);
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
            let vm = build(&raw, BuildOptions::at(0.0));
            assert_eq!(vm.nodes.len(), 1);
        }
    }
}
