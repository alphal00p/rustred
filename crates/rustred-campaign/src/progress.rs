//! Progress files derived from the reduced ledger (schema `rustred.campaign.progress.v1`), written atomically,
//! cheap to poll, and re-derivable by a second process: the terminal dashboard and the website read ONLY these
//! files (or call [`derive`] on a ledger they reduce themselves); a restart loses nothing.
//!
//! Files (CDIR/progress/):
//! * `campaign.json` (`"file": "campaign"`): campaign phase, mode, resources, jobs, nodes, items by ladder level and
//!   phase, rules, terminals, certification, closure defect, cost, ETA, attention summary, list of files;
//! * `nodes/<node>.json` (`"file": "node"`): one per node;
//! * `events.json` (`"file": "events"`): the bounded recent-events list ([`crate::state::RECENT_CAP`]);
//! * `terminals.json` (`"file": "terminals"`): every declared terminal per node, preferred / other counts;
//! * `attention.json` (`"file": "attention"`): items needing a person (reason code, text, command options) and
//!   operator controls not yet applied;
//! * `jobs.json` (`"file": "jobs"`, VOLATILE): running attempts with CPU set, RSS / cap, elapsed / budget,
//!   heartbeat, phase (resource samples are INPUTS from the supervisor, never ledger records);
//! * `heartbeat.json` (`"file": "heartbeat"`, VOLATILE): the supervisor heartbeat, rewritten on EVERY write
//!   even when nothing else changed (other files are skipped when unchanged).
//!
//! Per-item detail is a FUNCTION ([`item_detail`]). Resource numbers are INPUTS ([`Resources`]); everything else
//! comes from the reduced ledger. Unknown values are `null`, never 0. [`verify`] recomputes every number from the
//! raw ledger segments (M5) and fails when files are missing or older than the ledger tail (audit A M7).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::atomic::{self, AtomicError};
use crate::clock::Clock;
use crate::ledger::{self, CampaignMode, LedgerError};
use crate::state::{
    AttemptState, CampaignState, CertLevel, EvidenceLevel, ItemPhase, N_ITEM_PHASES, NodePhase,
    Rung,
};

pub const PROGRESS_SCHEMA: &str = "rustred.campaign.progress.v1";
pub const PROGRESS_DIR: &str = "progress";
/// Minimum number of finished jobs at a rung before its own quantiles are used by the ETA.
pub const ETA_MIN_SAMPLES: usize = 5;
/// Files whose content is a resource sample (presence + schema checked by `verify`, not re-derived).
pub const VOLATILE_FILES: [&str; 2] = ["heartbeat.json", "jobs.json"];

/// The supervisor's liveness (input; rewritten at least every 10 s into `heartbeat.json`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupervisorInfo {
    pub heartbeat_utc: Option<String>,
    pub started_utc: Option<String>,
    pub pid: Option<u32>,
    pub host: Option<String>,
    /// Seconds of the stop grace left while stopping.
    pub grace_left_s: Option<u64>,
    pub stopping: bool,
}

/// One running attempt as measured by the process layer (input).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveJob {
    pub job: String,
    pub attempt: u32,
    pub cpu_set: Vec<u32>,
    pub rss_bytes: Option<u64>,
    pub budget_mem_bytes: Option<u64>,
    pub seconds: Option<u64>,
    pub budget_seconds: Option<u64>,
    pub heartbeat_utc: Option<String>,
    pub pid: Option<u32>,
    pub pgid: Option<u32>,
    /// The child's own progress phase word (from its event stream).
    pub phase: Option<String>,
}

/// Resource numbers measured by the process layer (inputs of the derivation; copied verbatim). `None` = not
/// measured (written as `null`, audit A m10).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resources {
    pub cpus_in_use: Option<u32>,
    pub cpus_allowed: Option<u32>,
    /// Live child jobs (the ETA's parallelism); `None` = use the ledger's running attempts.
    pub live_jobs: Option<u32>,
    pub reserved_bytes: Option<u64>,
    pub measured_rss_bytes: Option<u64>,
    pub mem_cap_bytes: Option<u64>,
    pub host_mem_available_bytes: Option<u64>,
    pub host_floor_bytes: Option<u64>,
    /// Free bytes of the campaign directory's file system and of the large-output root.
    pub disk_free_bytes: Option<u64>,
    pub large_free_bytes: Option<u64>,
    /// The campaign's CPU budget (spec / command line), for the cost line.
    pub budget_cpu_hours: Option<u64>,
    pub sampled_utc: Option<String>,
    pub supervisor: Option<SupervisorInfo>,
    pub jobs: Vec<LiveJob>,
}

/// The derived content of every progress file, keyed by relative path.
pub type ProgressSet = BTreeMap<String, Value>;

fn round3(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
}

fn quantile(sorted: &[u64], q: f64) -> u64 {
    let n = sorted.len();
    let k = ((q * n as f64).ceil() as usize).clamp(1, n);
    sorted[k - 1]
}

/// Node file name: an INJECTIVE escape of the node name (audit-ledger2 m9: two names never share a file):
/// `[A-Za-z0-9.-]` stay, `_` becomes `__`, every other byte (UTF-8) becomes `_xx` (two lowercase hex digits); a
/// name of dots only (or empty) has its dots escaped too, so no node name leads outside `progress/nodes/` (U-11).
/// Plain names (letters, digits, `.`, `-`) keep their own file name.
pub fn node_file(node: &str) -> String {
    let dots_only = node.bytes().all(|b| b == b'.');
    let mut s = String::with_capacity(node.len() + 2);
    for b in node.bytes() {
        match b {
            b'_' => s.push_str("__"),
            b'.' if dots_only => s.push_str("_2e"),
            b if b.is_ascii_alphanumeric() || b == b'.' || b == b'-' => s.push(b as char),
            b => s.push_str(&format!("_{b:02x}")),
        }
    }
    if s.is_empty() {
        s.push('_');
    }
    format!("nodes/{s}.json")
}

/// ETA estimator (requirement R5): remaining items by ladder level x CPU cost quantiles per level / live
/// parallelism.
pub fn eta(state: &CampaignState, live_jobs: Option<u32>) -> Value {
    let mut remaining: BTreeMap<Rung, u64> = BTreeMap::new();
    for it in state.items.values() {
        if matches!(it.phase, ItemPhase::Open | ItemPhase::InProgress) {
            *remaining.entry(it.rung).or_default() += 1;
        }
    }
    let total_remaining: u64 = remaining.values().sum();
    let estimator = "sum over levels of remaining items (open + in_progress) x CPU-seconds quantile (p25 / p50 / p75) of finished jobs at that level, divided by live parallelism";
    let mut pooled: Vec<u64> = state.costs_ms.values().flatten().copied().collect();
    pooled.sort_unstable();
    let parallelism = match live_jobs {
        Some(n) if n > 0 => n as u64,
        _ => state
            .jobs
            .values()
            .filter(|j| matches!(j.state, AttemptState::Running | AttemptState::Launching))
            .count() as u64,
    };
    if total_remaining == 0 {
        return json!({"estimator": estimator, "remaining_items": 0, "cpu_seconds": {"low": 0.0, "mid": 0.0, "high": 0.0},
                      "wall_seconds": {"low": 0.0, "mid": 0.0, "high": 0.0}, "parallelism": parallelism,
                      "pooled_levels": [], "reason": "nothing left"});
    }
    let mut cpu = [0f64; 3];
    let mut pooled_levels = Vec::new();
    let mut by_level = serde_json::Map::new();
    for (rung, n) in &remaining {
        let own = state.costs_ms.get(rung).map(|v| v.len()).unwrap_or(0);
        let samples: Vec<u64> = if own >= ETA_MIN_SAMPLES {
            let mut v = state.costs_ms[rung].clone();
            v.sort_unstable();
            v
        } else if pooled.len() >= ETA_MIN_SAMPLES {
            pooled_levels.push(rung.to_string());
            pooled.clone()
        } else {
            return json!({"estimator": estimator, "remaining_items": total_remaining, "cpu_seconds": null, "wall_seconds": null,
                          "parallelism": parallelism, "pooled_levels": [],
                          "reason": format!("too little data: {} finished jobs in total, {ETA_MIN_SAMPLES} needed", pooled.len())});
        };
        let q = [
            quantile(&samples, 0.25),
            quantile(&samples, 0.5),
            quantile(&samples, 0.75),
        ];
        for k in 0..3 {
            cpu[k] += *n as f64 * q[k] as f64 / 1000.0;
        }
        by_level.insert(rung.to_string(), json!({"remaining": n, "samples": own, "p25_s": q[0] as f64 / 1000.0,
                                                 "p50_s": q[1] as f64 / 1000.0, "p75_s": q[2] as f64 / 1000.0}));
    }
    let cpu_v = json!({"low": round3(cpu[0]), "mid": round3(cpu[1]), "high": round3(cpu[2])});
    let (wall, reason) = if parallelism == 0 {
        (
            Value::Null,
            Value::String("no live parallelism (no job running)".into()),
        )
    } else {
        let p = parallelism as f64;
        (
            json!({"low": round3(cpu[0] / p), "mid": round3(cpu[1] / p), "high": round3(cpu[2] / p)}),
            Value::Null,
        )
    };
    json!({"estimator": estimator, "remaining_items": total_remaining, "by_level": by_level, "cpu_seconds": cpu_v,
           "wall_seconds": wall, "parallelism": parallelism, "pooled_levels": pooled_levels, "reason": reason})
}

fn phase_counts(counts: &[u64; N_ITEM_PHASES]) -> Value {
    let mut m = serde_json::Map::new();
    for (i, t) in ItemPhase::TAGS.iter().enumerate() {
        m.insert((*t).into(), json!(counts[i]));
    }
    Value::Object(m)
}

fn level_counts(counts: &[u64; 3]) -> Value {
    let mut m = serde_json::Map::new();
    for (i, l) in EvidenceLevel::ALL.iter().enumerate() {
        m.insert(l.word().into(), json!(counts[i]));
    }
    Value::Object(m)
}

fn ledger_block(state: &CampaignState, head: &str) -> Value {
    json!({"seq": state.last_seq, "head": head, "last_utc": state.last_utc})
}

fn envelope(
    file: &str,
    state: &CampaignState,
    head: &str,
    written_utc: &str,
) -> serde_json::Map<String, Value> {
    let mut m = serde_json::Map::new();
    m.insert("schema".into(), json!(PROGRESS_SCHEMA));
    m.insert("file".into(), json!(file));
    m.insert("campaign_id".into(), json!(state.campaign_id));
    m.insert("written_utc".into(), json!(written_utc));
    m.insert("source_of_truth".into(), json!("ledger"));
    m.insert("ledger".into(), ledger_block(state, head));
    m
}

/// The certification level all live rules of a node reach (`per-key`, `replay`, or null).
fn reached(n: &crate::state::NodeRec) -> Value {
    let live: u64 = n.accepted_by_level.iter().sum();
    if live == 0 {
        Value::Null
    } else if n.accepted_by_level[0] + n.accepted_by_level[1] == 0 {
        json!(CertLevel::PerKey.word())
    } else if n.accepted_by_level[0] == 0 {
        json!(CertLevel::Replay.word())
    } else {
        Value::Null
    }
}

/// Reference values (comparison only, U-9): numbers where they parse.
fn reference_value(r: &Option<BTreeMap<String, String>>) -> Value {
    match r {
        None => Value::Null,
        Some(m) => Value::Object(
            m.iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        v.parse::<u64>()
                            .map(|x| json!(x))
                            .unwrap_or_else(|_| json!(v)),
                    )
                })
                .collect(),
        ),
    }
}

/// Items needing a person (attention.json rows; `attention.top` of campaign.json).
fn attention_rows(state: &CampaignState) -> Vec<Value> {
    let mut rows = Vec::new();
    for (id, it) in &state.items {
        if let ItemPhase::Attention(code) = &it.phase {
            rows.push(json!({
                "id": id, "node": it.node, "label": it.label, "rung": it.rung, "code": code,
                "severity": "decision", "text": it.attention, "since_seq": it.updated_seq,
                "options": [format!("rr_campaign retry {id} --reason TEXT"), format!("rr_campaign skip {id} --reason TEXT"),
                            format!("rr_campaign why {id}")],
            }));
        }
    }
    for (cid, c) in &state.controls {
        if c.status == "requested" {
            rows.push(json!({
                "id": cid, "code": "control_pending", "severity": "warning",
                "text": format!("control {:?} of {:?} by {} not applied yet ({})", c.verb, c.target, c.by, c.reason),
                "since_seq": c.requested_seq, "options": [],
            }));
        }
    }
    rows
}

/// Derives every progress file from the reduced state, the ledger head and the resource inputs.
/// `written_utc` is the only field not determined by (state, head, resources).
pub fn derive(
    state: &CampaignState,
    head: &str,
    res: &Resources,
    written_utc: &str,
) -> ProgressSet {
    let mut out = ProgressSet::new();
    let mut by_rung: BTreeMap<Rung, [u64; N_ITEM_PHASES]> = BTreeMap::new();
    let mut by_node_rung: BTreeMap<&str, BTreeMap<Rung, [u64; N_ITEM_PHASES]>> = BTreeMap::new();
    let mut by_node_kind: BTreeMap<&str, BTreeMap<&str, (u64, u64)>> = BTreeMap::new();
    for it in state.items.values() {
        by_rung.entry(it.rung).or_insert([0; N_ITEM_PHASES])[it.phase.index()] += 1;
        by_node_rung
            .entry(it.node.as_str())
            .or_default()
            .entry(it.rung)
            .or_insert([0; N_ITEM_PHASES])[it.phase.index()] += 1;
        let k = by_node_kind
            .entry(it.node.as_str())
            .or_default()
            .entry(it.kind.as_str())
            .or_default();
        k.0 += 1;
        if !it.phase.is_open() {
            k.1 += 1;
        }
    }
    let items_by_phase = state.items_by_phase();
    let mut accepted_by_level = [0u64; 3];
    let mut nodes_by_phase: BTreeMap<&str, u64> =
        NodePhase::ALL.iter().map(|p| (p.tag(), 0)).collect();
    let mut nodes_by_closure: BTreeMap<&str, u64> =
        ["full", "boxed", "open"].iter().map(|k| (*k, 0)).collect();
    let (mut nodes_certified, mut nodes_provisional) = (0u64, 0u64);
    let mut asked: BTreeMap<&str, u64> = BTreeMap::new();
    let mut economy_before = crate::ledger::RuleEconomy::default();
    let mut economy_after = crate::ledger::RuleEconomy::default();
    let (mut rules_before, mut rules_after) = (0u64, 0u64);
    let mut defect_total = 0u64;
    let mut nodes_checked = 0u64;
    let mut nodes_with_defect = 0u64;
    let mut candidates = 0u64;
    let (mut pref_used, mut pref_listed, mut pref_listed_known, mut other_terms) =
        (0u64, 0u64, false, 0u64);
    for (id, n) in &state.nodes {
        for (a, b) in accepted_by_level.iter_mut().zip(n.accepted_by_level) {
            *a += b;
        }
        *nodes_by_phase.get_mut(n.state.phase.tag()).unwrap() += 1;
        *nodes_by_closure.get_mut(n.state.closure_kind()).unwrap() += 1;
        nodes_certified += n.state.certified() as u64;
        nodes_provisional += n.state.provisional() as u64;
        *asked.entry(n.state.required_cert().word()).or_default() += 1;
        if let Some(d) = n.state.last_defect {
            nodes_checked += 1;
            defect_total += d;
            if d > 0 {
                nodes_with_defect += 1;
            }
        }
        candidates += n.items_by_phase[ItemPhase::TerminalCandidate.index()];
        let tr = state.terminal_report(id);
        pref_used += tr.preferred_used;
        other_terms += tr.other;
        if let Some(l) = tr.preferred_listed {
            pref_listed += l;
            pref_listed_known = true;
        }
    }
    let accepted_total: u64 = accepted_by_level.iter().sum();
    let mut jobs_by_state: BTreeMap<&str, u64> =
        AttemptState::TAGS.iter().map(|t| (*t, 0)).collect();
    let mut classes: BTreeMap<&str, u64> = BTreeMap::new();
    let mut reserved_running = 0u64;
    let mut failed_attempts = 0u64;
    for j in state.jobs.values() {
        *jobs_by_state.get_mut(j.state.tag()).unwrap() += 1;
        failed_attempts += j.failures as u64;
        match j.state {
            AttemptState::SucceededSealed(c)
            | AttemptState::Failed(c)
            | AttemptState::BudgetStopped(c) => {
                *classes.entry(crate::state::class_word(c)).or_default() += 1
            }
            AttemptState::Launching | AttemptState::Running => reserved_running += j.reserved_bytes,
            _ => {}
        }
    }
    let mut cost_by_rung = serde_json::Map::new();
    for (r, v) in &state.costs_ms {
        let mut s = v.clone();
        s.sort_unstable();
        let total: u64 = s.iter().sum();
        cost_by_rung.insert(
            r.to_string(),
            json!({"jobs": s.len(), "cpu_seconds": total as f64 / 1000.0, "p25_s": quantile(&s, 0.25) as f64 / 1000.0,
                   "p50_s": quantile(&s, 0.5) as f64 / 1000.0, "p75_s": quantile(&s, 0.75) as f64 / 1000.0}),
        );
    }
    let mut rung_rows = serde_json::Map::new();
    for (r, c) in &by_rung {
        rung_rows.insert(r.to_string(), phase_counts(c));
    }
    for id in state.nodes.keys() {
        let e = state.node_economy(id);
        economy_before.add(&e.before);
        economy_after.add(&e.after);
        rules_before += e.rules_before;
        rules_after += e.rules_after;
    }
    let node_files: BTreeMap<&String, String> =
        state.nodes.keys().map(|k| (k, node_file(k))).collect();
    let attention = attention_rows(state);
    let supervisor_static = res
        .supervisor
        .as_ref()
        .map(|s| json!({"started_utc": s.started_utc, "pid": s.pid, "host": s.host}));
    let mut static_res = res.clone();
    static_res.jobs = Vec::new();
    static_res.supervisor = None;
    let mut c = envelope("campaign", state, head, written_utc);
    let campaign_rest = json!({
        "campaign": {
            "phase": state.phase.map(|p| p.tag()),
            "mode": match state.mode { CampaignMode::Close => "close", CampaignMode::Optimise => "optimise" },
            "paused": state.paused,
            "created_utc": state.created_utc,
            "spec_blake3": state.spec_blake3,
            "sessions": state.sessions,
            "crash_recoveries": state.crash_recoveries,
            "lock_takeovers": state.takeovers,
            "torn_tail_recoveries": state.torn_tail_recoveries,
            "forced_recoveries": state.forced_recoveries,
            "segments_opened": state.segments_opened,
            "orphans_killed": state.orphans_killed,
            "pending_operations": state.pending_ops.len(),
            "unsettled_jobs": state.unsettled_jobs().len(),
            "last_verify": state.last_verify.map(|(s, ok)| json!({"seq": s, "clean": ok})),
        },
        "supervisor": supervisor_static,
        "resources": static_res,
        "jobs": {"total": state.jobs.len(), "by_state": jobs_by_state, "settled_by_class": classes,
                 "reserved_bytes_running": reserved_running, "failed_attempts": failed_attempts},
        "nodes": {"total": state.nodes.len(), "by_phase": nodes_by_phase, "by_closure": nodes_by_closure,
                  "certified": nodes_certified, "provisional": nodes_provisional},
        "economy": {"rules_before": rules_before, "rules_after": rules_after, "before": economy_before,
                    "after": economy_after, "retired": state.rules.retired, "replaced": state.rules.replaced},
        "items": {"total": state.items.len(), "by_phase": phase_counts(&items_by_phase), "by_level": rung_rows},
        "rules": {"accepted": state.rules.accepted, "imported": state.rules.imported, "rejected": state.rules.rejected,
                  "rejected_by_reason": state.rules.rejected_by_reason, "upgrades": state.rules.upgrades,
                  "accepted_items_by_evidence": level_counts(&accepted_by_level)},
        "terminals": {"declared": state.terminals_total, "candidates": candidates,
                      "bound_per_node_default": state.terminal_bound,
                      "preferred": {"used": pref_used, "total": if pref_listed_known { json!(pref_listed) } else { Value::Null },
                                    "not_needed": if pref_listed_known { json!(pref_listed.saturating_sub(pref_used)) } else { Value::Null }},
                      "other": other_terms},
        "certified": {"share": if accepted_total == 0 { Value::Null } else { json!(round3(accepted_by_level[2] as f64 / accepted_total as f64)) },
                      "identity_proved": accepted_by_level[2], "accepted": accepted_total,
                      "basis": "accepted items at identity_proved / all accepted items (per-key share; see certification)"},
        "certification": {"asked": asked, "nodes_certified": nodes_certified,
                          "identity_proved": accepted_by_level[2], "identity_tested": accepted_by_level[1],
                          "accepted": accepted_total,
                          "basis": "certified = every live rule at the level its node's spec asks (FULL: replay, BOXED: per-key by default)"},
        "closure": {"defect_keys": if nodes_checked == 0 { Value::Null } else { json!(defect_total) },
                    "nodes_checked": nodes_checked, "nodes_with_defect": nodes_with_defect},
        "cost": {"cpu_seconds": state.cpu_ms_total as f64 / 1000.0, "by_level": cost_by_rung,
                 "budget_cpu_hours": res.budget_cpu_hours},
        "eta": eta(state, res.live_jobs),
        "attention": {"count": attention.len(), "top": attention.iter().take(20).cloned().collect::<Vec<_>>()},
        "files": {"events": "events.json", "terminals": "terminals.json", "attention": "attention.json",
                  "jobs": "jobs.json", "heartbeat": "heartbeat.json", "nodes": node_files},
    });
    if let Value::Object(m) = campaign_rest {
        c.extend(m);
    }
    out.insert("campaign.json".into(), Value::Object(c));
    let mut terminal_nodes = serde_json::Map::new();
    for (id, n) in &state.nodes {
        let e = state.node_economy(id);
        let tr = state.terminal_report(id);
        let mut rows = serde_json::Map::new();
        if let Some(m) = by_node_rung.get(id.as_str()) {
            for (r, c) in m {
                rows.insert(r.to_string(), phase_counts(c));
            }
        }
        let kinds: BTreeMap<&str, Value> = by_node_kind
            .get(id.as_str())
            .map(|m| {
                m.iter()
                    .map(|(k, (t, d))| (*k, json!({"total": t, "done": d})))
                    .collect()
            })
            .unwrap_or_default();
        let faces = kinds
            .get("face")
            .map(|v| json!({"total": v["total"], "closed": v["done"]}))
            .unwrap_or(Value::Null);
        let s = &n.state;
        let mut node = envelope("node", state, head, written_utc);
        let rest = json!({
            "node": id,
            "phase": s.phase.tag(),
            "closure_kind": s.closure_kind(),
            "certified": s.certified(),
            "provisional": s.provisional(),
            "envelope": s.closure.as_ref().map(|c| c.envelope),
            "certification": {"asked": s.required_cert().word(), "reached": reached(n),
                              "full": s.spec.certify_full.word(), "boxed": s.spec.certify_boxed.word()},
            "target": s.target,
            "spec_box": s.spec_box,
            "spec": {"full_attempt_cpu_ms": s.spec.full_attempt_cpu_ms, "full_attempt_fixer_rounds": s.spec.full_attempt_fixer_rounds,
                     "terminal_rung": s.spec.terminal_rung, "preferred_terminals": s.spec.preferred_terminals},
            "attempt_ended": s.attempt_ended,
            "attempt": {"cpu_ms_on_target": s.cpu_ms_on_target, "fixer_rounds_on_target": s.fixer_rounds_on_target},
            "blockers": {"full": s.full_blockers, "box": s.box_blockers},
            "closure": {"rounds": s.closure_rounds, "defect_keys": s.last_defect, "keyset": n.last_keyset,
                        "closure_keys": s.closure_keys, "rules_used": s.rules_used,
                        "rules_total": s.rules_total,
                        "used_share": match (s.rules_used, s.rules_total) {
                            (Some(u), Some(t)) if t > 0 => json!(round3(u as f64 / t as f64)),
                            _ => Value::Null,
                        },
                        "provisional": s.provisional(),
                        "binding": s.closure, "rule_set_digest": n.rule_set.digest(),
                        "dispatch_order": s.dispatch_order},
            "reopens": s.reopens,
            "reopen_cause": s.reopen_cause,
            "paused": n.paused,
            "economy": e,
            "terminals": {"declared": s.terminals, "preferred_declared": s.preferred_terminals, "bound": s.terminal_bound,
                          "candidates": n.items_by_phase[ItemPhase::TerminalCandidate.index()],
                          "preferred": {"used": tr.preferred_used, "total": tr.preferred_listed, "not_needed": tr.preferred_not_needed},
                          "other": tr.other},
            "items": {"by_phase": phase_counts(&n.items_by_phase), "by_level": rows, "by_kind": kinds,
                      "open": n.context().items_open},
            "faces_summary": faces,
            "accepted_by_evidence": level_counts(&n.accepted_by_level),
            "inputs": n.inputs,
            "reference": reference_value(&n.reference),
            "stale_reason": n.stale_reason,
            "cpu_seconds": n.cpu_ms as f64 / 1000.0,
        });
        if let Value::Object(m) = rest {
            node.extend(m);
        }
        out.insert(node_file(id), Value::Object(node));
        let list: Vec<Value> = state
            .items
            .iter()
            .filter(|(_, it)| it.node == *id && it.phase == ItemPhase::Terminal)
            .map(|(iid, it)| {
                json!({"item": iid, "label": it.label, "rung": it.rung, "keys": it.terminal_keys,
                       "keyset": it.terminal_keyset, "preferred": it.terminal_preferred,
                       "counted": it.terminal_counted, "ladder_exhausted": it.ladder_exhausted})
            })
            .collect();
        terminal_nodes.insert(id.clone(), json!({"report": tr, "terminals": list}));
    }
    let mut ev = envelope("events", state, head, written_utc);
    ev.insert("cap".into(), json!(crate::state::RECENT_CAP));
    ev.insert("events".into(), json!(state.recent));
    out.insert("events.json".into(), Value::Object(ev));
    let mut t = envelope("terminals", state, head, written_utc);
    t.insert("declared".into(), json!(state.terminals_total));
    t.insert("nodes".into(), Value::Object(terminal_nodes));
    out.insert("terminals.json".into(), Value::Object(t));
    let mut a = envelope("attention", state, head, written_utc);
    a.insert("count".into(), json!(attention.len()));
    a.insert("items".into(), json!(attention));
    out.insert("attention.json".into(), Value::Object(a));
    // volatile files
    let mut jobs = envelope("jobs", state, head, written_utc);
    let live: BTreeMap<&str, &LiveJob> = res.jobs.iter().map(|j| (j.job.as_str(), j)).collect();
    let list: Vec<Value> = state
        .jobs
        .iter()
        .filter(|(_, j)| !j.state.is_settled())
        .map(|(id, j)| {
            let it = state.items.get(&j.item);
            let l = live.get(id.as_str()).copied();
            json!({"job": id, "attempt": j.attempt, "state": j.state.tag(), "node": it.map(|i| &i.node),
                   "item": it.map(|i| &i.label), "item_id": j.item, "rung": j.rung, "kind": j.kind,
                   "run_dir": j.run_dir, "reserved_bytes": j.reserved_bytes,
                   "cpus": l.map(|l| l.cpu_set.len()), "cpu_set": l.map(|l| &l.cpu_set),
                   "rss_bytes": l.and_then(|l| l.rss_bytes), "budget_mem_bytes": l.and_then(|l| l.budget_mem_bytes),
                   "seconds": l.and_then(|l| l.seconds), "budget_seconds": l.and_then(|l| l.budget_seconds),
                   "heartbeat_utc": l.and_then(|l| l.heartbeat_utc.clone()),
                   "pid": l.and_then(|l| l.pid).or(j.pid), "pgid": l.and_then(|l| l.pgid),
                   "phase": l.and_then(|l| l.phase.clone())})
        })
        .collect();
    jobs.insert("sampled_utc".into(), json!(res.sampled_utc));
    jobs.insert("list".into(), json!(list));
    out.insert("jobs.json".into(), Value::Object(jobs));
    let mut hb = envelope("heartbeat", state, head, written_utc);
    hb.insert("supervisor".into(), json!(res.supervisor));
    out.insert("heartbeat.json".into(), Value::Object(hb));
    out
}

/// Per-item detail on demand (for `inspect` / `explain` and the dashboard's drill-down).
pub fn item_detail(state: &CampaignState, item: &str) -> Option<Value> {
    let it = state.items.get(item)?;
    let jobs: Vec<Value> = it
        .jobs
        .iter()
        .filter_map(|j| {
            state.jobs.get(j).map(|r| {
                json!({"job": j, "attempt": r.attempt, "state": r.state,
            "generation": r.generation, "stale": r.stale, "failures": r.failures,
            "cpu_seconds": r.cpu_ms as f64 / 1000.0, "run_dir": r.run_dir})
            })
        })
        .collect();
    let children: Vec<&String> = state
        .items
        .iter()
        .filter(|(_, c)| c.parent.as_deref() == Some(item))
        .map(|(k, _)| k)
        .collect();
    let why = match &it.phase {
        ItemPhase::Stale => format!(
            "stale since input {:?} changed (was {:?})",
            it.stale_input, it.stale_from
        ),
        ItemPhase::Rejected(r) => format!("rejected: {r}"),
        ItemPhase::Escalated(r) => format!("escalated to {r}"),
        ItemPhase::Accepted(l) => format!("accepted at {}", l.word()),
        ItemPhase::Attention(c) => format!(
            "needs attention ({c}): {}",
            it.attention.clone().unwrap_or_default()
        ),
        p => p.tag().to_string(),
    };
    Some(json!({"item": item, "record": it, "jobs": jobs, "children": children, "why": why}))
}

/// Writes progress files atomically. `heartbeat.json` is rewritten on every call; every other file is skipped
/// when its content without `written_utc` and the `ledger` block is unchanged (audit A m9), so a node file's
/// `ledger.seq` is the seq of its last content change.
#[derive(Debug, Default)]
pub struct ProgressWriter {
    last: BTreeMap<String, Value>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct WriteReport {
    pub written: Vec<String>,
    pub unchanged: usize,
}

/// The content compared for "unchanged": without `written_utc`, and without `ledger` except in campaign.json.
fn strip_volatile(rel: &str, v: &Value) -> Value {
    let mut v = v.clone();
    if let Some(o) = v.as_object_mut() {
        o.remove("written_utc");
        if rel != "campaign.json" {
            o.remove("ledger");
        }
    }
    v
}

impl ProgressWriter {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn write(
        &mut self,
        cdir: &Path,
        state: &CampaignState,
        head: &str,
        res: &Resources,
        clock: &dyn Clock,
    ) -> Result<WriteReport, AtomicError> {
        let set = derive(state, head, res, &clock.now_utc().to_iso());
        let dir = cdir.join(PROGRESS_DIR);
        let mut rep = WriteReport::default();
        for (rel, v) in set {
            let key = strip_volatile(&rel, &v);
            if rel != "heartbeat.json" && self.last.get(&rel) == Some(&key) {
                rep.unchanged += 1;
                continue;
            }
            let path = dir.join(&rel);
            if let Some(p) = path.parent() {
                fs::create_dir_all(p).map_err(|e| AtomicError::Io {
                    path: p.to_path_buf(),
                    op: "create dir",
                    source: e,
                })?;
            }
            let mut bytes = serde_json::to_vec_pretty(&v).expect("progress serialises");
            bytes.push(b'\n');
            atomic::write_atomic(&path, &bytes)?;
            self.last.insert(rel.clone(), key);
            rep.written.push(rel);
        }
        Ok(rep)
    }
}

/// One difference found by [`verify`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Difference {
    pub file: String,
    pub path: String,
    pub expected: String,
    pub found: String,
}

#[derive(Debug)]
pub struct VerifyReport {
    pub records: u64,
    pub segments: u32,
    pub snapshot_matches: Option<bool>,
    pub torn_tail_bytes: u64,
    pub files_checked: usize,
    /// Expected files that are absent.
    pub missing: Vec<String>,
    /// Files older than the ledger tail verified against: (file, seq it states).
    pub stale: Vec<(String, u64)>,
    pub differences: Vec<Difference>,
}

impl VerifyReport {
    pub fn clean(&self) -> bool {
        self.differences.is_empty()
            && self.missing.is_empty()
            && self.stale.is_empty()
            && self.snapshot_matches != Some(false)
            && self.torn_tail_bytes == 0
    }
}

fn diff_values(file: &str, path: &str, exp: &Value, got: &Value, out: &mut Vec<Difference>) {
    match (exp, got) {
        (Value::Object(a), Value::Object(b)) => {
            for (k, va) in a {
                let p = format!("{path}/{k}");
                match b.get(k) {
                    Some(vb) => diff_values(file, &p, va, vb, out),
                    None => out.push(Difference {
                        file: file.into(),
                        path: p,
                        expected: va.to_string(),
                        found: "<missing>".into(),
                    }),
                }
            }
            for k in b.keys().filter(|k| !a.contains_key(*k)) {
                out.push(Difference {
                    file: file.into(),
                    path: format!("{path}/{k}"),
                    expected: "<absent>".into(),
                    found: b[k].to_string(),
                });
            }
        }
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => {
            for (i, (va, vb)) in a.iter().zip(b).enumerate() {
                diff_values(file, &format!("{path}/{i}"), va, vb, out);
            }
        }
        _ if exp == got => {}
        _ => out.push(Difference {
            file: file.into(),
            path: path.into(),
            expected: exp.to_string(),
            found: got.to_string(),
        }),
    }
}

fn seq_of(v: &Value) -> Option<u64> {
    v.get("ledger")?.get("seq")?.as_u64()
}

/// Sum consistency checks of campaign.json (counts by state must add up to their totals).
fn check_sums(c: &Value, out: &mut Vec<Difference>) {
    for (total, parts) in [
        ("/jobs/total", "/jobs/by_state"),
        ("/items/total", "/items/by_phase"),
        ("/nodes/total", "/nodes/by_phase"),
        ("/nodes/total", "/nodes/by_closure"),
    ] {
        let t = c.pointer(total).and_then(Value::as_u64);
        let s: Option<u64> = c
            .pointer(parts)
            .and_then(Value::as_object)
            .map(|m| m.values().filter_map(Value::as_u64).sum());
        if t != s {
            out.push(Difference {
                file: "campaign.json".into(),
                path: parts.into(),
                expected: format!("sum = {total} ({t:?})"),
                found: format!("{s:?}"),
            });
        }
    }
}

/// M5 / M7 against the ledger END: see [`verify_against`].
pub fn verify(cdir: &Path) -> Result<VerifyReport, LedgerError> {
    verify_against(cdir, None)
}

/// Recomputes every number of the progress files from the RAW ledger segments (full replay from record 1,
/// sealed digests, framing, hash chain and commit mark checked, the snapshot compared) and reports:
/// * `missing`: expected files that are absent (campaign.json, events, terminals, attention, jobs, heartbeat,
///   one file per node), and `stale`: campaign.json stating a seq older than `target` (default: the ledger end);
/// * `differences`: campaign.json against the derivation at the seq it states (its resources taken from the
///   file); every other derived file against the same derivation without `written_utc` / `ledger` (its own
///   `ledger.seq` must not be after campaign.json's); files for unknown nodes; sum inconsistencies; a file
///   stating a seq beyond the ledger end. Volatile files (jobs, heartbeat) are checked for presence and schema.
pub fn verify_against(cdir: &Path, target: Option<u64>) -> Result<VerifyReport, LedgerError> {
    let dir = cdir.join(PROGRESS_DIR);
    let mut files: BTreeMap<String, Value> = BTreeMap::new();
    let mut list = Vec::new();
    collect_files(&dir, &dir, &mut list);
    files.extend(list);
    let mut differences = Vec::new();
    let mut missing = Vec::new();
    let mut stale = Vec::new();
    // ONE streaming pass over the ledger (audit B M4): the state at campaign.json's seq is captured on the way
    let cseq = files.get("campaign.json").and_then(seq_of).unwrap_or(0);
    let mut cap = ledger::Capture {
        at: cseq,
        got: None,
    };
    let mut chain = ledger::verify_chain_capture(cdir, Some(&mut cap))?;
    let target = target.unwrap_or(chain.records);
    let Some(campaign) = files.get("campaign.json") else {
        missing.push("campaign.json".into());
        return Ok(VerifyReport {
            records: chain.records,
            segments: chain.segments,
            snapshot_matches: chain.snapshot_matches,
            torn_tail_bytes: chain.torn_tail_bytes,
            files_checked: files.len(),
            missing,
            stale,
            differences,
        });
    };
    if cseq > chain.records {
        differences.push(Difference {
            file: "campaign.json".into(),
            path: "/ledger/seq".into(),
            expected: format!("<= {} (ledger end)", chain.records),
            found: format!(
                "{cseq} (progress ahead of the ledger: records lost or ledger truncated)"
            ),
        });
    }
    if cseq < target {
        stale.push(("campaign.json".into(), cseq));
    }
    let (st, head) = if cseq == chain.records {
        (std::mem::take(&mut chain.state), chain.head.clone())
    } else {
        cap.got
            .take()
            .unwrap_or_else(|| (CampaignState::default(), ledger::GENESIS.to_string()))
    };
    let res: Resources = campaign
        .get("resources")
        .cloned()
        .and_then(|r| serde_json::from_value(r).ok())
        .unwrap_or_default();
    let mut res = res;
    if let Some(s) = campaign.get("supervisor").filter(|s| !s.is_null()) {
        res.supervisor = Some(SupervisorInfo {
            started_utc: s
                .get("started_utc")
                .and_then(Value::as_str)
                .map(String::from),
            pid: s.get("pid").and_then(Value::as_u64).map(|p| p as u32),
            host: s.get("host").and_then(Value::as_str).map(String::from),
            ..Default::default()
        });
    }
    let written = campaign
        .get("written_utc")
        .and_then(|w| w.as_str())
        .unwrap_or("");
    let expected = derive(&st, &head, &res, written);
    check_sums(campaign, &mut differences);
    for (rel, exp) in &expected {
        let Some(got) = files.get(rel) else {
            missing.push(rel.clone());
            continue;
        };
        if VOLATILE_FILES.contains(&rel.as_str()) {
            if got.get("file") != exp.get("file") {
                differences.push(Difference {
                    file: rel.clone(),
                    path: "/file".into(),
                    expected: exp["file"].to_string(),
                    found: got.get("file").map(|v| v.to_string()).unwrap_or_default(),
                });
            }
            continue;
        }
        if rel == "campaign.json" {
            diff_values(rel, "", exp, got, &mut differences);
            continue;
        }
        if let Some(s) = seq_of(got)
            && s > cseq
        {
            differences.push(Difference {
                file: rel.clone(),
                path: "/ledger/seq".into(),
                expected: format!("<= {cseq} (campaign.json)"),
                found: s.to_string(),
            });
        }
        diff_values(
            rel,
            "",
            &strip_volatile(rel, exp),
            &strip_volatile(rel, got),
            &mut differences,
        );
    }
    for rel in files.keys().filter(|k| !expected.contains_key(*k)) {
        differences.push(Difference {
            file: rel.clone(),
            path: String::new(),
            expected: "<no such file at this seq>".into(),
            found: "file present".into(),
        });
    }
    Ok(VerifyReport {
        records: chain.records,
        segments: chain.segments,
        snapshot_matches: chain.snapshot_matches,
        torn_tail_bytes: chain.torn_tail_bytes,
        files_checked: files.len(),
        missing,
        stale,
        differences,
    })
}

fn collect_files(root: &Path, dir: &Path, out: &mut Vec<(String, Value)>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut entries: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for p in entries {
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if name.starts_with('.') {
            continue;
        }
        if p.is_dir() {
            collect_files(root, &p, out);
        } else if name.ends_with(".json")
            && let Ok(bytes) = atomic::read_capped(&p, 64 << 20)
            && let Ok(v) = serde_json::from_slice::<Value>(&bytes)
            && v.get("schema").and_then(|s| s.as_str()) == Some(PROGRESS_SCHEMA)
        {
            let rel = p.strip_prefix(root).unwrap().to_string_lossy().into_owned();
            out.push((rel, v));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantiles_are_nearest_rank() {
        let v = [1, 2, 3, 4, 5, 6, 7, 8];
        assert_eq!(quantile(&v, 0.25), 2);
        assert_eq!(quantile(&v, 0.5), 4);
        assert_eq!(quantile(&v, 0.75), 6);
        assert_eq!(quantile(&[9], 0.5), 9);
        assert_eq!(node_file("T162"), "nodes/T162.json");
        assert_eq!(node_file("T162/x y"), "nodes/T162_2fx_20y.json");
        assert_eq!(node_file(".."), "nodes/_2e_2e.json");
        assert_eq!(node_file(""), "nodes/_.json");
        // injective: names that sanitised to one file before (audit-ledger2 m9) get two
        assert_ne!(node_file("fam/x"), node_file("fam_x"));
        assert_ne!(node_file("_2e"), node_file("."));
    }
}
