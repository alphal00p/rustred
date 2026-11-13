//! Progress files derived from the reduced ledger (schema `rustred.campaign.progress.v1`), written atomically,
//! cheap to poll, and re-derivable by a second process: the terminal dashboard and the website read ONLY these
//! files (or call [`derive`] on a ledger they reduce themselves); a restart loses nothing.
//!
//! Files (CDIR/progress/):
//! * `campaign.json` (`"file": "campaign"`): campaign phase, resources, jobs, nodes, items by ladder level and
//!   phase, rules, terminals, certified share, closure defect, cost, ETA, list of node files;
//! * `nodes/<node>.json` (`"file": "node"`): one per node;
//! * `events.json` (`"file": "events"`): the bounded recent-events list ([`crate::state::RECENT_CAP`]).
//!
//! Per-item detail is a FUNCTION ([`item_detail`]), not a file, to keep file counts sane at tens of thousands
//! of items. Resource numbers are INPUTS ([`Resources`], filled by the process layer); everything else comes
//! from the reduced ledger. [`verify`] recomputes every number from the raw ledger segments (M5).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::atomic::{self, AtomicError};
use crate::clock::Clock;
use crate::ledger::{self, LedgerError};
use crate::state::{AttemptState, CampaignState, EvidenceLevel, ItemPhase, NodePhase, Rung};

pub const PROGRESS_SCHEMA: &str = "rustred.campaign.progress.v1";
pub const PROGRESS_DIR: &str = "progress";
/// Minimum number of finished jobs at a rung before its own quantiles are used by the ETA.
pub const ETA_MIN_SAMPLES: usize = 5;

/// Resource numbers measured by the process layer (inputs of the derivation; copied verbatim into
/// `campaign.json`). Zero means "not measured".
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resources {
    pub cpus_in_use: u32,
    pub cpus_allowed: u32,
    /// Live child jobs (the ETA's parallelism); 0 = use the ledger's running attempts.
    pub live_jobs: u32,
    pub reserved_bytes: u64,
    pub measured_rss_bytes: u64,
    pub mem_cap_bytes: u64,
    pub host_mem_available_bytes: u64,
    pub host_floor_bytes: u64,
    pub sampled_utc: Option<String>,
}

/// The derived content of every progress file, keyed by relative path (`campaign.json`, `events.json`,
/// `nodes/<node>.json`).
pub type ProgressSet = BTreeMap<String, Value>;

fn round3(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
}

fn quantile(sorted: &[u64], q: f64) -> u64 {
    // nearest rank, deterministic
    let n = sorted.len();
    let k = ((q * n as f64).ceil() as usize).clamp(1, n);
    sorted[k - 1]
}

/// Node file name: characters outside `[A-Za-z0-9_.-]` become `_`.
pub fn node_file(node: &str) -> String {
    let s: String = node
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "_.-".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("nodes/{s}.json")
}

/// ETA estimator (requirement R5): remaining items by ladder level x running median CPU cost per level /
/// live parallelism; low / high from the 25th / 75th percentiles. A level with fewer than
/// [`ETA_MIN_SAMPLES`] finished jobs uses the pooled samples of all levels (listed in `pooled_levels`); with
/// fewer than that in total, or no live parallelism, the wall-clock range is null with a reason.
pub fn eta(state: &CampaignState, live_jobs: u32) -> Value {
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
    let parallelism = if live_jobs > 0 {
        live_jobs as u64
    } else {
        state
            .jobs
            .values()
            .filter(|j| matches!(j.state, AttemptState::Running | AttemptState::Launching))
            .count() as u64
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

fn phase_counts(counts: &[u64; 10]) -> Value {
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

/// Derives every progress file from the reduced state, the ledger head and the resource inputs.
/// `written_utc` is the only field not determined by (state, head, resources).
pub fn derive(
    state: &CampaignState,
    head: &str,
    res: &Resources,
    written_utc: &str,
) -> ProgressSet {
    let mut out = ProgressSet::new();
    // by rung x phase over all items
    let mut by_rung: BTreeMap<Rung, [u64; 10]> = BTreeMap::new();
    let mut by_node_rung: BTreeMap<&str, BTreeMap<Rung, [u64; 10]>> = BTreeMap::new();
    for it in state.items.values() {
        by_rung.entry(it.rung).or_insert([0; 10])[it.phase.index()] += 1;
        by_node_rung
            .entry(it.node.as_str())
            .or_default()
            .entry(it.rung)
            .or_insert([0; 10])[it.phase.index()] += 1;
    }
    let items_by_phase = state.items_by_phase();
    let mut accepted_by_level = [0u64; 3];
    let mut nodes_by_phase: BTreeMap<&str, u64> =
        NodePhase::ALL.iter().map(|p| (p.tag(), 0)).collect();
    let mut nodes_by_closure: BTreeMap<&str, u64> =
        ["full", "boxed", "open"].iter().map(|k| (*k, 0)).collect();
    let mut nodes_certified = 0u64;
    let mut economy_before = crate::ledger::RuleEconomy::default();
    let mut economy_after = crate::ledger::RuleEconomy::default();
    let (mut rules_before, mut rules_after) = (0u64, 0u64);
    let mut defect_total = 0u64;
    let mut nodes_checked = 0u64;
    let mut nodes_with_defect = 0u64;
    let mut candidates = 0u64;
    for n in state.nodes.values() {
        for (a, b) in accepted_by_level.iter_mut().zip(n.accepted_by_level) {
            *a += b;
        }
        *nodes_by_phase.get_mut(n.state.phase.tag()).unwrap() += 1;
        *nodes_by_closure.get_mut(n.state.closure_kind()).unwrap() += 1;
        if n.state.certified() {
            nodes_certified += 1;
        }
        if let Some(d) = n.state.last_defect {
            nodes_checked += 1;
            defect_total += d;
            if d > 0 {
                nodes_with_defect += 1;
            }
        }
        candidates += n.items_by_phase[ItemPhase::TerminalCandidate.index()];
    }
    let accepted_total: u64 = accepted_by_level.iter().sum();
    let certified_share = if accepted_total == 0 {
        Value::Null
    } else {
        json!(round3(accepted_by_level[2] as f64 / accepted_total as f64))
    };
    let mut jobs_by_state: BTreeMap<&str, u64> =
        AttemptState::TAGS.iter().map(|t| (*t, 0)).collect();
    let mut classes: BTreeMap<&str, u64> = BTreeMap::new();
    let mut reserved_running = 0u64;
    for j in state.jobs.values() {
        *jobs_by_state.get_mut(j.state.tag()).unwrap() += 1;
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
    let campaign = json!({
        "schema": PROGRESS_SCHEMA,
        "file": "campaign",
        "campaign_id": state.campaign_id,
        "written_utc": written_utc,
        "source_of_truth": "ledger",
        "ledger": ledger_block(state, head),
        "campaign": {
            "phase": state.phase.map(|p| p.tag()),
            "created_utc": state.created_utc,
            "spec_blake3": state.spec_blake3,
            "sessions": state.sessions,
            "crash_recoveries": state.crash_recoveries,
            "lock_takeovers": state.takeovers,
            "torn_tail_recoveries": state.torn_tail_recoveries,
            "segments_opened": state.segments_opened,
            "pending_operations": state.pending_ops.len(),
            "unsettled_jobs": state.unsettled_jobs().len(),
        },
        "resources": res,
        "jobs": {"total": state.jobs.len(), "by_state": jobs_by_state, "settled_by_class": classes,
                 "reserved_bytes_running": reserved_running},
        "nodes": {"total": state.nodes.len(), "by_phase": nodes_by_phase, "by_closure": nodes_by_closure,
                  "certified": nodes_certified},
        "economy": {"rules_before": rules_before, "rules_after": rules_after, "before": economy_before,
                    "after": economy_after, "retired": state.rules.retired},
        "items": {"total": state.items.len(), "by_phase": phase_counts(&items_by_phase), "by_level": rung_rows},
        "rules": {"accepted": state.rules.accepted, "rejected": state.rules.rejected,
                  "rejected_by_reason": state.rules.rejected_by_reason, "upgrades": state.rules.upgrades,
                  "accepted_items_by_evidence": level_counts(&accepted_by_level)},
        "terminals": {"declared": state.terminals_total, "candidates": candidates, "bound_per_node_default": state.terminal_bound},
        "certified": {"share": certified_share, "identity_proved": accepted_by_level[2], "accepted": accepted_total,
                      "basis": "accepted items at identity_proved / all accepted items"},
        "closure": {"defect_keys": if nodes_checked == 0 { Value::Null } else { json!(defect_total) },
                    "nodes_checked": nodes_checked, "nodes_with_defect": nodes_with_defect},
        "cost": {"cpu_seconds": state.cpu_ms_total as f64 / 1000.0, "by_level": cost_by_rung},
        "eta": eta(state, res.live_jobs),
        "files": {"events": "events.json", "nodes": node_files},
    });
    out.insert("campaign.json".into(), campaign);
    for (id, n) in &state.nodes {
        let e = state.node_economy(id);
        let mut rows = serde_json::Map::new();
        if let Some(m) = by_node_rung.get(id.as_str()) {
            for (r, c) in m {
                rows.insert(r.to_string(), phase_counts(c));
            }
        }
        let node = json!({
            "schema": PROGRESS_SCHEMA,
            "file": "node",
            "campaign_id": state.campaign_id,
            "written_utc": written_utc,
            "source_of_truth": "ledger",
            "ledger": ledger_block(state, head),
            "node": id,
            "phase": n.state.phase.tag(),
            "closure_kind": n.state.closure_kind(),
            "certified": n.state.certified(),
            "target": n.state.target,
            "spec_box": n.state.spec_box,
            "attempt_ended": n.state.attempt_ended,
            "blockers": {"full": n.state.full_blockers, "box": n.state.box_blockers},
            "closure": {"rounds": n.state.closure_rounds, "defect_keys": n.state.last_defect, "keyset": n.last_keyset,
                        "closure_keys": n.state.closure_keys, "rules_used": n.state.rules_used,
                        "used_share": match (n.state.rules_used, n.accepted_by_level.iter().sum::<u64>()) {
                            (Some(u), t) if t > 0 => json!(round3(u as f64 / t as f64)),
                            _ => Value::Null,
                        }},
            "economy": e,
            "terminals": {"declared": n.state.terminals, "bound": n.state.terminal_bound,
                          "candidates": n.items_by_phase[ItemPhase::TerminalCandidate.index()]},
            "items": {"by_phase": phase_counts(&n.items_by_phase), "by_level": rows, "open": n.context().items_open},
            "accepted_by_evidence": level_counts(&n.accepted_by_level),
            "inputs": n.inputs,
            "stale_reason": n.stale_reason,
            "cpu_seconds": n.cpu_ms as f64 / 1000.0,
        });
        out.insert(node_file(id), node);
    }
    let events = json!({
        "schema": PROGRESS_SCHEMA,
        "file": "events",
        "campaign_id": state.campaign_id,
        "written_utc": written_utc,
        "source_of_truth": "ledger",
        "ledger": ledger_block(state, head),
        "cap": crate::state::RECENT_CAP,
        "events": state.recent,
    });
    out.insert("events.json".into(), events);
    out
}

/// Per-item detail on demand (for `inspect` / `explain` and the dashboard's drill-down).
pub fn item_detail(state: &CampaignState, item: &str) -> Option<Value> {
    let it = state.items.get(item)?;
    let jobs: Vec<Value> = it
        .jobs
        .iter()
        .filter_map(|j| state.jobs.get(j).map(|r| json!({"job": j, "attempt": r.attempt, "state": r.state, "cpu_seconds": r.cpu_ms as f64 / 1000.0, "run_dir": r.run_dir})))
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
        p => p.tag().to_string(),
    };
    Some(json!({"item": item, "record": it, "jobs": jobs, "children": children, "why": why}))
}

/// Writes progress files atomically, skipping files whose content (without `written_utc`) is unchanged since
/// the last write by this writer.
#[derive(Debug, Default)]
pub struct ProgressWriter {
    last: BTreeMap<String, Value>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct WriteReport {
    pub written: Vec<String>,
    pub unchanged: usize,
}

fn strip_volatile(v: &Value) -> Value {
    let mut v = v.clone();
    if let Some(o) = v.as_object_mut() {
        o.remove("written_utc");
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
            let key = strip_volatile(&v);
            if self.last.get(&rel) == Some(&key) {
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
    pub differences: Vec<Difference>,
}

impl VerifyReport {
    pub fn clean(&self) -> bool {
        self.differences.is_empty()
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

/// M5: recomputes every number of the progress files on disk from the RAW ledger segments (full replay from
/// record 1, sealed digests and the hash chain checked, the snapshot compared) and reports every difference.
/// Each file is compared with the derivation at the ledger seq it states; `resources` and `written_utc` are
/// taken from the file (inputs). A file stating a seq beyond the ledger's end is reported (a truncated ledger).
pub fn verify(cdir: &Path) -> Result<VerifyReport, LedgerError> {
    let chain = ledger::verify_chain(cdir)?;
    let records = ledger::read_records(cdir)?;
    let dir = cdir.join(PROGRESS_DIR);
    let mut files: Vec<(String, Value)> = Vec::new();
    collect_files(&dir, &dir, &mut files);
    let mut differences = Vec::new();
    // seqs needed
    let mut wanted: BTreeMap<u64, Option<(CampaignState, String)>> = BTreeMap::new();
    for (rel, v) in &files {
        match v
            .get("ledger")
            .and_then(|l| l.get("seq"))
            .and_then(|s| s.as_u64())
        {
            Some(s) if s <= chain.records => {
                wanted.insert(s, None);
            }
            Some(s) => differences.push(Difference {
                file: rel.clone(),
                path: "/ledger/seq".into(),
                expected: format!("<= {} (ledger end)", chain.records),
                found: format!(
                    "{s} (progress ahead of the ledger: records lost or ledger truncated)"
                ),
            }),
            None => differences.push(Difference {
                file: rel.clone(),
                path: "/ledger/seq".into(),
                expected: "a number".into(),
                found: "<missing>".into(),
            }),
        }
    }
    let mut st = CampaignState::default();
    let mut head = ledger::GENESIS.to_string();
    let mut it = records.iter().peekable();
    let seqs: Vec<u64> = wanted.keys().copied().collect();
    for s in seqs {
        while st.last_seq < s {
            let Some(r) = it.next() else { break };
            st.apply_record(r).map_err(LedgerError::Replay)?;
            head = ledger::line_hash(&serde_json::to_vec(r).expect("records serialise"));
        }
        wanted.insert(s, Some((st.clone(), head.clone())));
    }
    for (rel, v) in &files {
        let Some(s) = v
            .get("ledger")
            .and_then(|l| l.get("seq"))
            .and_then(|s| s.as_u64())
        else {
            continue;
        };
        let Some(Some((state, head))) = wanted.get(&s) else {
            continue;
        };
        let res: Resources = v
            .get("resources")
            .cloned()
            .and_then(|r| serde_json::from_value(r).ok())
            .unwrap_or_default();
        let written = v.get("written_utc").and_then(|w| w.as_str()).unwrap_or("");
        let set = derive(state, head, &res, written);
        match set.get(rel) {
            Some(exp) => diff_values(rel, "", exp, v, &mut differences),
            None => differences.push(Difference {
                file: rel.clone(),
                path: String::new(),
                expected: "<no such file at this seq>".into(),
                found: "file present".into(),
            }),
        }
    }
    Ok(VerifyReport {
        records: chain.records,
        segments: chain.segments,
        snapshot_matches: chain.snapshot_matches,
        torn_tail_bytes: chain.torn_tail_bytes,
        files_checked: files.len(),
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
        assert_eq!(node_file("T162/x y"), "nodes/T162_x_y.json");
    }
}
