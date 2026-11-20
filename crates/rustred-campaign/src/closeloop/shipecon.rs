//! The SHIP ECONOMY driver (lane shipecon; interfaces/shipecon.md; core `economy::ship`): after a node is closed, per
//! table rule of its material: bytes vs certified keys served, THE GUARD (the bounded recipe solved and certified at
//! every certified key the rule serves: engine `solve-recipe` jobs through the solve-recipe adapter and the process
//! layer's launcher (`adapters::run_attempt`: own session, PDEATHSIG, verified kill, memory cap), each settled attempt
//! through the closeloop's accept chain against the SHIPPED dispatch), the decision per rule, then the CLOSING CHECK of
//! the shipped spec (the import emulates the engine's order: table first match, then recipe records / entries, then
//! the typed refusal), with rules reverted to the table where the check fails behind them. Report mode (default)
//! writes the proposal and the records; apply mode writes the shipped artifact.
//!
//! ITERABLE: the records of an earlier run (`previous`) are read: a rule with the same inputs and decision settings is
//! not measured or decided again; with other settings it is re-decided from its recorded trials and only new trials
//! run. Run it inside a private pid namespace (L10). Generic (L8): node, paths, order, bounds are configuration.

use super::accept::{BATCH_SCHEMA, BatchFile};
use super::termmin::{AcceptBits, accept_point, cost_of};
use crate::adapters::job::{InputFile, JobAdapter, Program, RunOptions, run_attempt};
use crate::adapters::solve_recipe::{
    BatchPolicy, RecipeInput, SolveRecipeAdapter, SolveRecipeConfig, SolveRequest as AdReq,
};
use crate::launch::RealLauncher;
use crate::resources::{Sampler, SamplerConfig};
use rustred_campaign_contract::layout::RunLayout;
use rustred_campaign_contract::seal::verify_stream_handle;
use rustred_campaign_core::closure::relations::DispatchSlot;
use rustred_campaign_core::economy::ship::{
    self as ship, Candidacy, Census, CensusBuilder, ClosingCheck, ClosureView,
    ECONOMY_RECORD_SCHEMA, EconomyRecord, KeyTrial, Outcome, RECORD_KIND, REPORT_SCHEMA, Reuse,
    RuleDecision, RuleRef, ShipMode, ShipPolicy, ShippedRecipe, TableTime,
};
use rustred_campaign_core::import::recipe_steps::{RecipeForm, recipes_for_steps};
use rustred_campaign_core::import::spec::FileRef;
use rustred_campaign_core::import::statement::all_rows;
use rustred_campaign_core::import::{
    CheckOptions, Evidence, ImportedSet, RowStatus, Server, TableSetSpec, check, load,
};
use rustred_campaign_core::termmin::machine::Cost;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// What the driver needs (all data).
#[derive(Clone)]
pub struct ShipRunConfig {
    pub node: String,
    pub import_spec: PathBuf,
    pub import_base: PathBuf,
    /// The node's statement rows (`statement.jsonl` of the tool's closing check); None = the check runs here.
    pub statement_rows: Option<PathBuf>,
    pub policy: ShipPolicy,
    pub program: Program,
    pub family: PathBuf,
    pub order: String,
    /// Concurrent solve jobs (unpinned: the caller's unit bounds the CPUs).
    pub parallel: usize,
    pub threads_per_job: u32,
    pub cap_bytes: u64,
    /// Wall cap of one solve child (terminate after it: typed refusal).
    pub wall_cap_s: u64,
    pub work_root: PathBuf,
    pub out_dir: PathBuf,
    pub check_threads: usize,
    pub licence_env: Option<String>,
    /// `records.jsonl` of an earlier run (iteration).
    pub previous: Option<PathBuf>,
    /// Measured table times (`rr_shipecon_table` output, JSONL `{record_sha256, load_s, apply_s, instantiated_bytes,
    /// tool}`); rules without one get the cost-model estimate.
    pub table_times: Option<PathBuf>,
    /// Keys of the engine dispatch cross-check (every migrated key + a stride sample of the closure up to this many).
    pub engine_probe_sample: usize,
    /// Format of `statement_rows`: `tool` (the tool's statement.jsonl) | `ship_keys` (JSON arrays `[key text, kind,
    /// rule index | null, in_box, ..]`: a ship's per-key server list; `rule_index` maps a rule index to its record sha256).
    pub statement_format: String,
    /// The table's index (`{"index", "kind": "rule", "sha256"}` per line) for `ship_keys`.
    pub rule_index: Option<PathBuf>,
    /// `full` (the import loads and checks the shipped spec) | `local` (the material is a SELECTION of the node's rules:
    /// coverage is proved locally: every migrated key served by its certified relation whose top right-hand side lies in
    /// the certified set; requires `coverage.require_rhs_in_certified_set`).
    pub check_mode: String,
    /// `rr_node first-match` output (`{key, position}` lines) on the node's SHIPPED table (the leaving rules out) for the
    /// migrated keys: a key with a position is taken by a later table rule (the engine's own dispatch; replaces the
    /// tentative set, which in local mode sees only the selected rules). Given: every examined rule is re-decided.
    pub later_rule_probe: Option<PathBuf>,
    /// Local mode: `rr_node first-match` output on the FINAL shipped table (only the migrating rules out) for the migrated
    /// keys: a key the engine still gives to a table rule would make its entry DEAD (never shipped: the key goes to
    /// `later_table_rule`; local mode then reverts its rule).
    pub final_probe: Option<PathBuf>,
    /// `trials.jsonl` of an interrupted run (the per-job checkpoint): a trial is reused when the recipe generated now
    /// has the same sha256 and its verdict is deterministic or was measured under the same bound.
    pub resume_trials: Option<PathBuf>,
    /// An earlier run's `ship/solved.jsonl` (answer lines of trials recorded without them) and work root (its sealed
    /// results streams, for the engine records of reused certified trials).
    pub previous_solved: Option<PathBuf>,
    pub previous_work: Option<PathBuf>,
    /// Test stand: the program is `rr_fake_child`; per step and key the right-hand side of a SOLVED answer (absent:
    /// NO_RELATION_IN_SEEDS).
    pub fake: Option<ShipFake>,
    /// `rr_node first-match --table ship/probe/table.jsonl --keys ship/probe/keys.jsonl` output of an earlier run's
    /// probe (compared key by key with the import's dispatch of the shipped spec).
    pub engine_first_match: Option<PathBuf>,
    pub verbose: bool,
}

fn sha256_hex(b: &[u8]) -> String {
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(b);
    h.finish_hex()
}

fn now() -> String {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("t={t}")
}

fn write(p: &Path, b: &[u8]) -> Result<(), String> {
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    std::fs::write(p, b).map_err(|e| format!("{}: {e}", p.display()))
}

fn jsonl<T: serde::Serialize>(items: &[T]) -> Vec<u8> {
    let mut out = Vec::new();
    for i in items {
        out.extend_from_slice(
            serde_json::to_string(i)
                .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
                .as_bytes(),
        );
        out.push(b'\n');
    }
    out
}

/// Test stand of the driver (the fake child emulates kind solve-recipe).
#[derive(Clone, Debug, Default)]
pub struct ShipFake {
    pub answers: BTreeMap<String, BTreeMap<Vec<i32>, Vec<Vec<i64>>>>,
    pub ms: u64,
}

impl ShipFake {
    pub fn script(
        &self,
        step: &str,
        keys: &[Vec<i32>],
    ) -> rustred_campaign_contract::fake::FakeScript {
        use rustred_campaign_contract::classify::recipe::Verdict;
        use rustred_campaign_contract::fake::{FakeOp, FakeScript, OnSigterm, RssPlan};
        let mut ops = vec![FakeOp::Prepare { ms: 5 }];
        for k in keys {
            let (verdict, rhs) = match self.answers.get(step).and_then(|m| m.get(k)) {
                Some(rhs) => (Verdict::Solved, rhs.clone()),
                None => (Verdict::NoRelationInSeeds, vec![]),
            };
            ops.push(FakeOp::Relation {
                ms: self.ms,
                verdict,
                rhs,
            });
        }
        ops.push(FakeOp::End {
            stop: None,
            verdict: None,
        });
        FakeScript {
            script: ops,
            on_sigterm: OnSigterm::Graceful,
            slow_ms: 0,
            start_delay_ms: 0,
            rss: RssPlan::default(),
            helpers: vec![],
            faults: vec![],
            time_scale: 1.0,
            volatile_jitter: false,
        }
    }
}

/// One solve job: one recipe (one explicit seed set, common slots fixed) at the keys of one rule it is assigned.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Job {
    pub id: String,
    /// Census index of the examined rule.
    pub rule: usize,
    pub step: String,
    pub recipe_id: String,
    pub recipe_path: PathBuf,
    pub recipe_sha256: String,
    pub keys: Vec<Vec<i32>>,
}

struct Done {
    job: Job,
    result: Result<(String, Vec<String>, String, String, Cost), String>,
}

/// The closure view of a check: keys, start keys, terminals, open count, blocking words.
fn view_of(
    set: &ImportedSet,
    res: &rustred_campaign_core::import::CheckResult,
) -> Result<ClosureView, String> {
    let rows = all_rows(set, res)?;
    let mut v = ClosureView {
        blocking: res.blocking(),
        ..Default::default()
    };
    for r in &rows {
        v.keys.push(r.key.clone());
        if r.in_start {
            v.start.push(r.key.clone());
        }
        match r.status {
            RowStatus::Terminal => v.terminals.push(r.key.clone()),
            RowStatus::Open => {
                v.open += 1;
                v.open_keys.push(r.key.clone());
            }
            RowStatus::Served => {}
        }
    }
    v.keys.sort();
    v.start.sort();
    v.terminals.sort();
    Ok(v)
}

/// The rules of the dispatch (identity, bytes), in table / record order.
fn rules_of(set: &ImportedSet) -> Vec<(RuleRef, u64)> {
    let mut out = vec![];
    for t in &set.rules {
        for i in t {
            out.push((
                RuleRef {
                    source: i.source.clone(),
                    table: i.table_name.clone(),
                    label: i.label,
                    ordinal: i.ordinal,
                    record_sha256: i.record_sha256.clone(),
                    table_sha256: set.table_digests.get(&i.table_name).cloned(),
                },
                i.bytes,
            ));
        }
    }
    out
}

/// The original statement's service of every key (the served-key-set check's start).
type ServedMap = BTreeMap<Vec<i32>, ship::ServedAt>;

/// The census and the "before" view: from the statement rows when given, else from a check of the original spec.
fn census_and_before(
    cfg: &ShipRunConfig,
    set: &ImportedSet,
    say: &dyn Fn(String),
) -> Result<(Census, ClosureView, Value, ServedMap), String> {
    let t0 = Instant::now();
    let mut b = CensusBuilder::new(rules_of(set), cfg.policy.candidates.max_keys_per_rule)?;
    let mut v = ClosureView::default();
    // the ORIGINAL statement's service of every key (served at its level / terminal): the served-key-set check; not
    // kept in local mode (the shipped spec is not checked there)
    let mut served: BTreeMap<Vec<i32>, ship::ServedAt> = BTreeMap::new();
    let keep_served = cfg.check_mode != "local";
    let source;
    let index_map: BTreeMap<i64, String> = match (&cfg.rule_index, cfg.statement_format.as_str()) {
        (Some(ip), "ship_keys") => {
            let f = std::fs::File::open(ip).map_err(|e| format!("{}: {e}", ip.display()))?;
            let mut m = BTreeMap::new();
            for l in std::io::BufReader::new(f).lines() {
                let l = l.map_err(|e| e.to_string())?;
                let v: Value =
                    serde_json::from_str(&l).map_err(|e| format!("{}: {e}", ip.display()))?;
                if v["kind"] == "rule"
                    && let (Some(i), Some(h)) = (v["index"].as_i64(), v["sha256"].as_str())
                {
                    m.insert(i, h.to_string());
                }
            }
            m
        }
        (None, "ship_keys") => return Err("statement_format ship_keys needs rule_index".into()),
        (_, "tool") => BTreeMap::new(),
        (_, f) => return Err(format!("statement_format {f:?}: tool | ship_keys")),
    };
    match &cfg.statement_rows {
        Some(p) if cfg.statement_format == "ship_keys" => {
            let f = std::fs::File::open(p).map_err(|e| format!("{}: {e}", p.display()))?;
            for l in std::io::BufReader::with_capacity(1 << 20, f).lines() {
                let l = l.map_err(|e| format!("{}: {e}", p.display()))?;
                if l.trim().is_empty() {
                    continue;
                }
                let r: Value =
                    serde_json::from_str(&l).map_err(|e| format!("{}: {e}", p.display()))?;
                let key = r[0]
                    .as_str()
                    .and_then(rustred_campaign_core::assemble::parse_key_text)
                    .ok_or_else(|| format!("{}: key {}", p.display(), r[0]))?;
                let in_start = r[3] == true;
                let sha = (r[1] == "table")
                    .then(|| {
                        r[2].as_i64()
                            .and_then(|i| index_map.get(&i))
                            .map(String::as_str)
                    })
                    .flatten();
                b.add(&key, in_start, sha);
                if r[1] == "terminal" {
                    v.terminals.push(key.clone());
                }
                if in_start {
                    v.start.push(key.clone());
                }
                v.keys.push(key);
            }
            // procfix7 round 19 (landing, int8b-gate review (ii) shipecon.rs:324) / INT8-STAGE2 section 8 row 6): a size never read is never recorded
            let bytes = crate::fsx::presence("closeloop.input", p)
                .metadata()
                .map_err(|e| format!("{}: {e}", p.display()))?
                .len();
            source = json!({"statement_rows": p.display().to_string(), "format": "ship_keys",
                "bytes": bytes,
                "rule_index": cfg.rule_index.as_ref().map(|x| x.display().to_string())});
        }
        Some(p) => {
            let f = std::fs::File::open(p).map_err(|e| format!("{}: {e}", p.display()))?;
            for l in std::io::BufReader::with_capacity(1 << 20, f).lines() {
                let l = l.map_err(|e| format!("{}: {e}", p.display()))?;
                if l.trim().is_empty() {
                    continue;
                }
                let r: Value =
                    serde_json::from_str(&l).map_err(|e| format!("{}: {e}", p.display()))?;
                let key: Vec<i32> = serde_json::from_value(r["key"].clone())
                    .map_err(|e| format!("{}: key: {e}", p.display()))?;
                let in_start = r["in_start"] == true;
                let sha = (r["status"] == "served" && r["server"]["kind"] == "rule")
                    .then(|| r["server"]["record_sha256"].as_str())
                    .flatten();
                b.add(&key, in_start, sha);
                if r["status"] == "open"
                    && let Some(os) = r["open"]["rule"]["record_sha256"].as_str()
                {
                    b.add_open(os);
                }
                if keep_served {
                    match r["status"].as_str() {
                        Some("terminal") => {
                            served.insert(key.clone(), ship::ServedAt::Terminal);
                        }
                        Some("served") => {
                            let l: rustred_campaign_core::import::Level =
                                serde_json::from_value(r["evidence"].clone()).unwrap_or_default();
                            served.insert(key.clone(), ship::ServedAt::Served(l));
                        }
                        _ => {}
                    }
                }
                match r["status"].as_str() {
                    Some("terminal") => v.terminals.push(key.clone()),
                    Some("open") => {
                        v.open += 1;
                        v.open_keys.push(key.clone());
                    }
                    _ => {}
                }
                if in_start {
                    v.start.push(key.clone());
                }
                v.keys.push(key);
            }
            // procfix7 round 19 (landing, int8b-gate review (ii) shipecon.rs:375) / row 7): as above
            let bytes = crate::fsx::presence("closeloop.input", p)
                .metadata()
                .map_err(|e| format!("{}: {e}", p.display()))?
                .len();
            source = json!({"statement_rows": p.display().to_string(), "bytes": bytes});
        }
        None => {
            let ev =
                Evidence::from_spec(set, &cfg.import_base).map_err(|e| format!("evidence: {e}"))?;
            let res = check(
                set,
                &ev,
                &CheckOptions {
                    threads: cfg.check_threads.max(1),
                    tier: None,
                    max_rounds: None,
                },
            )
            .map_err(|e| format!("check: {e}"))?;
            for r in all_rows(set, &res)? {
                if keep_served {
                    match r.status {
                        RowStatus::Terminal => {
                            served.insert(r.key.clone(), ship::ServedAt::Terminal);
                        }
                        RowStatus::Served => {
                            served.insert(
                                r.key.clone(),
                                ship::ServedAt::Served(r.evidence.unwrap_or_default()),
                            );
                        }
                        RowStatus::Open => {}
                    }
                }
                let sha = match (&r.status, &r.server) {
                    (RowStatus::Served, Some(Server::Rule { table, record })) => {
                        Some(set.rules[*table][*record].record_sha256.as_str())
                    }
                    _ => None,
                };
                b.add(&r.key, r.in_start, sha);
                if r.status == RowStatus::Open
                    && let Some((t, rr)) = r.open_rule
                {
                    b.add_open(&set.rules[t][rr].record_sha256);
                }
            }
            v = view_of(set, &res)?;
            source = json!({"check": {"spec_digest": res.spec_digest, "blocking": res.blocking(),
                "seconds": res.seconds}});
        }
    }
    v.keys.sort();
    v.start.sort();
    v.terminals.sort();
    say(format!(
        "census: {} rows in {:.1} s; {} keys, {} start, {} terminals, {} open",
        b.rows,
        t0.elapsed().as_secs_f64(),
        v.keys.len(),
        v.start.len(),
        v.terminals.len(),
        v.open
    ));
    Ok((b.finish(), v, source, served))
}

/// Previous records by record sha256.
fn previous_records(p: Option<&Path>) -> Result<BTreeMap<String, EconomyRecord>, String> {
    let mut out = BTreeMap::new();
    let Some(p) = p else { return Ok(out) };
    let f = std::fs::File::open(p).map_err(|e| format!("{}: {e}", p.display()))?;
    for l in std::io::BufReader::new(f).lines() {
        let l = l.map_err(|e| e.to_string())?;
        if l.trim().is_empty() {
            continue;
        }
        let r: EconomyRecord =
            serde_json::from_str(&l).map_err(|e| format!("{}: {e}", p.display()))?;
        if r.schema == ECONOMY_RECORD_SCHEMA && r.kind == RECORD_KIND {
            out.insert(r.subject.record_sha256.clone(), r);
        }
    }
    Ok(out)
}

/// Measured table times by record sha256.
fn table_times(p: Option<&Path>) -> Result<BTreeMap<String, TableTime>, String> {
    let mut out = BTreeMap::new();
    let Some(p) = p else { return Ok(out) };
    let f = std::fs::File::open(p).map_err(|e| format!("{}: {e}", p.display()))?;
    for l in std::io::BufReader::new(f).lines() {
        let l = l.map_err(|e| e.to_string())?;
        if l.trim().is_empty() {
            continue;
        }
        let v: Value = serde_json::from_str(&l).map_err(|e| format!("{}: {e}", p.display()))?;
        let Some(sha) = v["record_sha256"].as_str() else {
            continue;
        };
        if v["error"].is_string() {
            continue;
        }
        let m = |k: &str| -> BTreeMap<String, Value> {
            v[k].as_object()
                .map(|o| o.iter().map(|(a, b)| (a.clone(), b.clone())).collect())
                .unwrap_or_default()
        };
        out.insert(
            sha.to_string(),
            TableTime {
                measured: true,
                load_s: v["load_s"].as_f64().unwrap_or(0.0),
                apply_s: m("apply_s")
                    .into_iter()
                    .filter_map(|(k, x)| x.as_f64().map(|x| (k, x)))
                    .collect(),
                instantiated_bytes: m("instantiated_bytes")
                    .into_iter()
                    .filter_map(|(k, x)| x.as_u64().map(|x| (k, x)))
                    .collect(),
                tool: v["tool"].as_str().map(str::to_string),
            },
        );
    }
    Ok(out)
}

/// The header of the frame table (node order: coordinate priority).
fn coordinate_priority(set: &ImportedSet) -> Option<Vec<usize>> {
    Some(set.cmp.equivalent_descriptor().coordinate_priority)
}

/// (recipe id, recipe document, its keys).
type GroupRecipe = (String, Value, Vec<Vec<i32>>);

/// The recipes of one (rule, step) at the keys: grouped by explicit seed set (the core's step grammar); each group's
/// recipe has the slots common to its keys fixed (the acceptance chain reads `domain.fixed`) and is assigned its keys.
fn recipes_for(
    p: &ShipPolicy,
    set: &ImportedSet,
    rule: &RuleRef,
    step: &str,
    keys: &[Vec<i32>],
) -> Result<Vec<GroupRecipe>, String> {
    let lines: Vec<bool> = set.frame.mask().to_vec();
    let cp = coordinate_priority(set);
    let (lp, np) = rustred_campaign_core::termmin::priorities(&lines, cp.as_deref());
    let lp = p.recipe.line_priority.clone().unwrap_or(lp);
    let np = p.recipe.numerator_priority.clone().unwrap_or(np);
    let assignments: Vec<(String, Vec<i32>)> =
        keys.iter().map(|k| (step.to_string(), k.clone())).collect();
    let prefix = format!(
        "shipecon/{}/",
        rule.record_sha256.get(..16).unwrap_or(&rule.record_sha256)
    );
    let g = recipes_for_steps(&assignments, &lines, &lp, &np, RecipeForm::V1_2, &prefix)
        .map_err(|e| format!("recipe step {step}: {e}"))?;
    let mut out = vec![];
    for r in g {
        let mut doc = r.recipe.clone();
        let n = lines.len();
        let fixed: Vec<Value> = (0..n)
            .map(|i| {
                let v0 = r.keys[0][i];
                if r.keys.iter().all(|k| k[i] == v0) {
                    json!(v0)
                } else {
                    Value::Null
                }
            })
            .collect();
        let rule_word = doc["relation_rule"]
            .as_str()
            .unwrap_or("first_admissible")
            .to_string();
        if let Some(o) = doc.as_object_mut() {
            o.remove("relation_rule");
            o.insert(
                "domain".into(),
                json!({"fixed": fixed, "equations": [], "exceptions": []}),
            );
            o["provenance"]["relation_rule"] = Value::String(rule_word.clone());
            o["provenance"]["shipecon_rule"] = json!(rule.record_sha256);
        }
        // the engine's default relation rule is m1_first; a first_admissible step needs the v1.2 field
        if rule_word != "m1_first"
            && let Some(o) = doc.as_object_mut()
        {
            o.insert("relation_rule".into(), Value::String(rule_word));
        }
        out.push((r.id.clone(), doc, r.keys.clone()));
    }
    Ok(out)
}

/// The served-key-set check: the shipped spec with every key the original statement serves (or declares terminal) in
/// its START set, loaded and checked; each such key's service compared with the original's.
fn served_set_check(
    cfg: &ShipRunConfig,
    sspec: &TableSetSpec,
    before: &BTreeMap<Vec<i32>, ship::ServedAt>,
    it: u32,
) -> Result<ship::ServedSetCheck, String> {
    let kp = cfg
        .out_dir
        .join(format!("ship/served_start.iter{it}.jsonl"));
    let mut text = String::with_capacity(before.len() * 48);
    for k in before.keys() {
        text.push_str(&json!(k).to_string());
        text.push('\n');
    }
    write(&kp, text.as_bytes())?;
    let mut spec = sspec.clone();
    spec.start = serde_json::from_value(
        json!({"path": kp.display().to_string(), "format": "keys_jsonl",
        "sha256": sha256_hex(text.as_bytes())}),
    )
    .map_err(|e| format!("served-set start: {e}"))?;
    let set = load(&spec, &cfg.import_base).map_err(|e| format!("served-set load: {e}"))?;
    let mut ev = Evidence::from_spec(&set, &cfg.import_base)
        .map_err(|e| format!("served-set evidence: {e}"))?;
    ev.accept_ledger_verified_files();
    let res = check(
        &set,
        &ev,
        &CheckOptions {
            threads: cfg.check_threads.max(1),
            tier: None,
            max_rounds: None,
        },
    )
    .map_err(|e| format!("served-set check: {e}"))?;
    let mut after: BTreeMap<Vec<i32>, ship::ServedAt> = BTreeMap::new();
    for r in all_rows(&set, &res)? {
        match r.status {
            RowStatus::Terminal => {
                after.insert(r.key, ship::ServedAt::Terminal);
            }
            RowStatus::Served => {
                after.insert(
                    r.key,
                    ship::ServedAt::Served(r.evidence.unwrap_or_default()),
                );
            }
            RowStatus::Open => {}
        }
    }
    Ok(ship::compare_served(before, &after))
}

/// Run one solve job through the adapter and the launcher (worker thread).
fn launch(
    cfg: &ShipRunConfig,
    job: Job,
    sampler: Arc<Sampler>,
    tx: mpsc::Sender<Done>,
) -> Result<(), String> {
    let adcfg = SolveRecipeConfig {
        program: cfg.program.clone(),
        family: InputFile::from_file(&cfg.family).map_err(|e| format!("{e:?}"))?,
        order: cfg.order.clone(),
        batch: BatchPolicy {
            max_keys_per_job: job.keys.len().max(1),
            max_threads: cfg.threads_per_job.max(1),
            min_keys_per_thread: 1,
        },
        max_trace_rows: cfg.policy.recipe.max_trace_rows,
        key_budget_s: cfg.policy.recipe.key_budget_s,
        expect: None,
        determinism_id: None,
        heartbeat_s: 10,
        stop_grace_s: 30,
        package: None,
        live_repair: false,
        licence_env: cfg.licence_env.clone(),
        fake: cfg.fake.as_ref().map(|f| f.script(&job.step, &job.keys)),
    };
    let reqs: Vec<AdReq> = job
        .keys
        .iter()
        .map(|k| {
            Ok(AdReq {
                recipe: RecipeInput {
                    id: job.recipe_id.clone(),
                    file: InputFile::from_file(&job.recipe_path).map_err(|e| format!("{e:?}"))?,
                    digest: None,
                    sha256: Some(job.recipe_sha256.clone()),
                },
                key: k.iter().map(|&x| x as i64).collect(),
                tag: Some(job.id.clone()),
                context: None,
            })
        })
        .collect::<Result<_, String>>()?;
    let work = cfg.work_root.join(crate::closeloop::ids::safe(&job.id));
    let opts = RunOptions {
        cpus: vec![],
        memory_cap_bytes: Some(cfg.cap_bytes),
        licence_env: cfg.licence_env.clone(),
        wall_cap: Some(Duration::from_secs(cfg.wall_cap_s)),
        grace: Some(Duration::from_secs(30)),
    };
    std::thread::Builder::new()
        .name(format!("shipecon-{}", job.id))
        .spawn(move || {
            let res = (|| {
                std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
                let ad = SolveRecipeAdapter::new(adcfg);
                let plans = ad.plan(&reqs).map_err(|e| format!("{e:?}"))?;
                let plan = plans.first().ok_or("no plan")?;
                if plans.len() != 1 {
                    return Err(format!("{} plans for one job", plans.len()));
                }
                // procfix7 round 19: a launcher that cannot hold its (ad-hoc) id is refused, never run without its lock
                let mut l =
                    RealLauncher::new(sampler).map_err(|e| format!("launcher refused: {e}"))?;
                let (out, rep) = run_attempt(&mut l, &ad, &reqs, plan, &work, 1, &opts)
                    .map_err(|e| format!("{e:?}"))?;
                let run_dir = rep.prepared.spec.run_dir.clone();
                let cost = cost_of(Path::new(&run_dir), &out.job_id);
                let c = &out.classification;
                let decided = matches!((c.row, c.sub.as_str()), (1, "") | (28, "") | (29, ""));
                if !decided {
                    return Ok((
                        format!(
                            "row {}{} ({:?}): {}; refusal {:?}",
                            c.row,
                            c.sub,
                            c.kind,
                            c.why,
                            out.refusal.as_ref().map(|r| &r.why)
                        ),
                        vec![],
                        String::new(),
                        run_dir,
                        cost,
                    ));
                }
                let layout = RunLayout::new(&run_dir);
                let (_, h) = verify_stream_handle(
                    &layout.root,
                    &RunLayout::out_rel("results.jsonl"),
                    &["end"],
                )
                .map_err(|e| format!("results stream: {e:?}"))?;
                let lines: Vec<String> = h
                    .canonical_lines()
                    .map_err(|e| format!("canonical lines: {e:?}"))?
                    .into_iter()
                    .map(|b| String::from_utf8_lossy(&b).into_owned())
                    .collect();
                let p = layout.root.join(RunLayout::out_rel("results.jsonl"));
                let b3 = crate::atomic::blake3_file(&p)
                    .map(|x| x.1)
                    .unwrap_or_default();
                Ok((p.display().to_string(), lines, b3, run_dir, cost))
            })();
            let _ = tx.send(Done { job, result: res });
        })
        .map_err(|e| format!("worker thread: {e}"))?;
    Ok(())
}

/// Per key of a results file: the engine's volatile `seconds` and the record (raw line).
///
/// procfix7 round 19 (landing, int8b-gate review (ii) shipecon.rs:736)): a results file that is not there has no records; one that cannot be READ
/// (open or mid-file) is an ERROR (it was "no records": a certified trial lost its record silently).
fn raw_records(results: &str) -> Result<BTreeMap<Vec<i32>, Value>, String> {
    let mut out = BTreeMap::new();
    let f = match crate::fsx::open("closeloop.input", Path::new(results)) {
        crate::fsx::Read::Content(f) => f,
        crate::fsx::Read::Absent => return Ok(out),
        crate::fsx::Read::Failed(e) => return Err(format!("{results}: {e}")),
    };
    {
        for l in std::io::BufReader::new(f).lines() {
            let l = l.map_err(|e| format!("{results}: {e}"))?;
            if let Ok(v) = serde_json::from_str::<Value>(&l)
                && v["type"] == "relation"
                && let Ok(k) = serde_json::from_value::<Vec<i32>>(v["key"].clone())
            {
                out.insert(k, v);
            }
        }
    }
    Ok(out)
}

/// Members of an engine relation record that a `rustred.relation-entry.v1` body drops (volatile / batch-bound).
const ENTRY_DROPPED: [&str; 4] = ["seconds", "vmhwm_kb", "ordinal", "tag"];

/// The entry body of a record (canonical JSON, volatile members dropped).
fn entry_body(rec: &Value) -> Value {
    let mut b = rec.clone();
    if let Some(o) = b.as_object_mut() {
        for k in ENTRY_DROPPED {
            o.remove(k);
        }
    }
    b
}

/// Run the ship economy to its end; returns the report (also written to `out_dir/report.json`).
/// The result of one guard job: (results path, verified canonical lines, results blake3, run dir, cost), or why not.
pub type JobResult = Result<(String, Vec<String>, String, String, Cost), String>;

/// The state of one ship-economy run between its steps (prepare -> rounds of guard jobs -> finish): the direct driver
/// ([`run_ship`]) and the supervised work source drive the same steps.
pub struct ShipState {
    p: ShipPolicy,
    t0: Instant,
    isp: TableSetSpec,
    set0: ImportedSet,
    load_s: f64,
    census: Census,
    before: ClosureView,
    census_source: Value,
    before_served: BTreeMap<Vec<i32>, ship::ServedAt>,
    cand: Vec<Candidacy>,
    prev: BTreeMap<String, EconomyRecord>,
    ttimes: BTreeMap<String, TableTime>,
    plans: BTreeMap<usize, Reuse>,
    n_exam: usize,
    n_unchanged: usize,
    taken_by_table: BTreeSet<Vec<i32>>,
    slot_a: DispatchSlot,
    certified_keys: std::collections::HashSet<Vec<i32>>,
    bits: AcceptBits,
    resumed: BTreeMap<(String, String, Vec<i32>), KeyTrial>,
    /// Trials per examined rule (census index).
    pub trials: BTreeMap<usize, Vec<KeyTrial>>,
    solved_lines: Vec<String>,
    records_by_key: BTreeMap<Vec<i32>, Value>,
    recipe_files: BTreeMap<String, (PathBuf, String, u64)>,
    jobs_log: Vec<Value>,
    guard_cpu_s: f64,
    guard_wall_s: f64,
    bound: Value,
}

impl ShipState {
    /// Load, census (statement rows or a check), candidacy, plans from the previous records, the tentative shipped
    /// dispatch (keys a later table rule takes), the accept bits, the resumable checkpoint.
    pub fn prepare(cfg: &ShipRunConfig) -> Result<ShipState, String> {
        let t0 = Instant::now();
        let p = &cfg.policy;
        p.validate()?;
        let say = |m: String| {
            if cfg.verbose {
                eprintln!("[shipecon {} {}] {m}", cfg.node, now());
            }
        };
        std::fs::create_dir_all(&cfg.out_dir).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(&cfg.work_root).map_err(|e| e.to_string())?;
        let isp = TableSetSpec::read(&cfg.import_spec).map_err(|e| format!("import spec: {e}"))?;
        say(format!("loading {}", cfg.import_spec.display()));
        let set0 = load(&isp, &cfg.import_base).map_err(|e| format!("import load: {e}"))?;
        let load_s = t0.elapsed().as_secs_f64();
        let (census, before, census_source, before_served) = census_and_before(cfg, &set0, &say)?;
        let cand = ship::candidacy(p, &census);
        let prev = previous_records(cfg.previous.as_deref())?;
        let ttimes = table_times(cfg.table_times.as_deref())?;
        // plan per examined rule
        let mut plans: BTreeMap<usize, Reuse> = BTreeMap::new();
        for (i, c) in cand.iter().enumerate() {
            if *c == Candidacy::Examine {
                plans.insert(
                    i,
                    ship::plan(
                        p,
                        &census.rules[i],
                        prev.get(&census.rules[i].rule.record_sha256),
                    ),
                );
            }
        }
        // a rule decided earlier with an ESTIMATED table time and measured now: re-decided (its trials reused)
        for (i, r) in plans.iter_mut() {
            let sha = &census.rules[*i].rule.record_sha256;
            if *r == Reuse::Unchanged
                && ttimes.contains_key(sha)
                && prev
                    .get(sha)
                    .is_some_and(|x| x.table_time.as_ref().is_none_or(|t| !t.measured))
            {
                *r = Reuse::Continue {
                    trials: prev[sha].trials.clone(),
                    to_try: vec![],
                };
            }
        }
        // the table-time request (rr_shipecon_table): the examined rules and their certified keys
        let req: Vec<Value> = plans
            .keys()
            .map(|&i| {
                json!({"record_sha256": census.rules[i].rule.record_sha256, "label": census.rules[i].rule.label,
                       "bytes": census.rules[i].bytes,
                       "keys": census.rules[i].keys.iter().map(|(k, _)| k.clone()).collect::<Vec<_>>()})
            })
            .collect();
        write(&cfg.out_dir.join("table_time_request.jsonl"), &jsonl(&req))?;
        write(
            &cfg.out_dir.join("census.json"),
            &serde_json::to_vec_pretty(&json!({"totals": census.totals(), "rules": census.rules.iter().enumerate()
                .map(|(i, r)| json!({"rule": r.rule, "bytes": r.bytes, "in": r.served_in_start, "out": r.served_outside_start,
                                     "candidacy": cand[i]}))
                .collect::<Vec<_>>()}))
            .map_err(|e| e.to_string())?,
        )?;
        let n_exam = plans.len();
        let n_unchanged = plans.values().filter(|r| **r == Reuse::Unchanged).count();
        say(format!(
            "census: {} rules, {} bytes; examined {n_exam} ({n_unchanged} unchanged since the previous run), kept aside {}",
            census.rules.len(),
            census.total_bytes,
            cand.iter().filter(|c| **c == Candidacy::KeptAside).count()
        ));
        // the tentative shipped dispatch: every examined and kept-aside rule removed (the accept chain's dispatch check)
        let tentative: Vec<RuleDecision> = cand
            .iter()
            .enumerate()
            .filter(|(_, c)| matches!(c, Candidacy::Examine | Candidacy::KeptAside))
            .map(|(i, _)| {
                let mut d = ship::decide_unexamined(&census.rules[i], &Candidacy::KeptAside);
                d.outcome = Outcome::KeptAside;
                d.why = "tentative removal (guard dispatch)".into();
                d
            })
            .collect();
        let tspec = ship::shipped_spec(&isp, &tentative, &[], &[], p.stored.precedence)?;
        let tspec_path = cfg.out_dir.join("work/tentative.spec.json");
        write(
            &tspec_path,
            &serde_json::to_vec_pretty(&ship::spec_json(&tspec)).map_err(|e| e.to_string())?,
        )?;
        let set_t = load(&tspec, &cfg.import_base).map_err(|e| format!("tentative load: {e}"))?;
        // the keys a LATER table rule takes once the examined / kept-aside rules leave the shipped table
        let mut taken_by_table: BTreeSet<Vec<i32>> = BTreeSet::new();
        for (i, c) in cand.iter().enumerate() {
            if matches!(c, Candidacy::Examine) {
                for (k, _) in &census.rules[i].keys {
                    if matches!(
                        set_t.disp.first_match(k),
                        Ok(rustred_campaign_core::cover::Dispatch::Covered(_))
                    ) {
                        taken_by_table.insert(k.clone());
                    }
                }
            }
        }
        if let Some(lp) = &cfg.later_rule_probe {
            let text = std::fs::read_to_string(lp).map_err(|e| format!("{}: {e}", lp.display()))?;
            let examined: BTreeSet<&Vec<i32>> = cand
                .iter()
                .enumerate()
                .filter(|(_, c)| matches!(c, Candidacy::Examine))
                .flat_map(|(i, _)| census.rules[i].keys.iter().map(|(k, _)| k))
                .collect();
            for l in text.lines().filter(|l| !l.trim().is_empty()) {
                let v: Value =
                    serde_json::from_str(l).map_err(|e| format!("{}: {e}", lp.display()))?;
                let k: Vec<i32> =
                    serde_json::from_value(v["key"].clone()).map_err(|e| e.to_string())?;
                if examined.contains(&k) && !v["position"].is_null() {
                    taken_by_table.insert(k);
                }
            }
            for (i, r) in plans.iter_mut() {
                if *r == Reuse::Unchanged {
                    *r = Reuse::Continue {
                        trials: prev
                            .get(&census.rules[*i].rule.record_sha256)
                            .map(|x| x.trials.clone())
                            .unwrap_or_default(),
                        to_try: vec![],
                    };
                }
            }
        }
        say(format!(
            "shipped dispatch: {} examined keys taken by a later table rule{}",
            taken_by_table.len(),
            if cfg.later_rule_probe.is_some() {
                " (engine probe)"
            } else {
                ""
            }
        ));
        drop(set_t);
        // the accept chain certifies each relation as such (no table before it): the slot before the first table
        let set_a = &set0;
        let first_table = {
            let tables = set_a.disp.tables();
            set_a
                .disp
                .priority()
                .first()
                .map(|&t| tables[t].name.clone())
                .ok_or("no table in the dispatch")?
        };
        let slot_a = DispatchSlot::BeforeTable(first_table);
        let certified_keys: std::collections::HashSet<Vec<i32>> =
            before.keys.iter().cloned().collect();
        // trials: reusable ones from the plan, new ones by rounds of steps
        let mut trials: BTreeMap<usize, Vec<KeyTrial>> = BTreeMap::new();
        for (&i, r) in &plans {
            let have = match r {
                Reuse::Continue { trials, .. } => trials.clone(),
                Reuse::Unchanged => prev
                    .get(&census.rules[i].rule.record_sha256)
                    .map(|x| x.trials.clone())
                    .unwrap_or_default(),
                Reuse::Fresh => vec![],
            };
            trials.insert(i, have);
        }
        let prog_bytes = std::fs::read(&cfg.program.path)
            .map_err(|e| format!("{}: {e}", cfg.program.path.display()))?;
        let bits = AcceptBits {
            boxed: true,
            determinism_id: None,
            binary_blake3: blake3::hash(&prog_bytes).to_hex().to_string(),
            binary_sha256: sha256_hex(&prog_bytes),
            binary_path: cfg.program.path.display().to_string(),
            family_path: cfg.family.display().to_string(),
            family_blake3: crate::atomic::blake3_file(&cfg.family)
                .map_err(|e| e.to_string())?
                .1,
            order: cfg.order.clone(),
            max_trace_rows: p.recipe.max_trace_rows,
            key_budget_s: p.recipe.key_budget_s,
            // the default pole objective (rule 21, 2026-11-19: size first, d = 4 regularity a tie-break): the SOLVED lines carry its annotation
            pole_objective: Some(rustred_campaign_core::poles::PoleObjective::default()),
            // int8 (wireflow x shipecon): wireflow (h) added the engine's per-key pole order as the choice's profile,
            // default false = "no profile"; the guard's annotation stays as lane shipecon built and tested it
            pole_profiles: false,
        };
        let mut resumed: BTreeMap<(String, String, Vec<i32>), KeyTrial> = BTreeMap::new();
        if let Some(rp) = &cfg.resume_trials {
            let f = std::fs::File::open(rp).map_err(|e| format!("{}: {e}", rp.display()))?;
            for l in std::io::BufReader::new(f).lines().map_while(Result::ok) {
                // a torn last line (a crash while appending) is skipped
                let Ok(v) = serde_json::from_str::<Value>(&l) else {
                    continue;
                };
                let (Some(sha), Ok(t)) = (
                    v["record_sha256"].as_str(),
                    serde_json::from_value::<KeyTrial>(v["trial"].clone()),
                ) else {
                    continue;
                };
                resumed.insert((sha.to_string(), t.step.clone(), t.key.clone()), t);
            }
            say(format!(
                "checkpoint: {} trials read from {}",
                resumed.len(),
                rp.display()
            ));
        }
        let solved_lines: Vec<String> = vec![];
        let records_by_key: BTreeMap<Vec<i32>, Value> = BTreeMap::new();
        let recipe_files: BTreeMap<String, (PathBuf, String, u64)> = BTreeMap::new();
        let jobs_log: Vec<Value> = vec![];
        let guard_cpu_s = 0.0f64;
        let guard_wall_s = 0.0f64;
        let bound = p.bound_value();
        let first_table_slot = slot_a;
        Ok(ShipState {
            p: cfg.policy.clone(),
            t0,
            isp,
            set0,
            load_s,
            census,
            before,
            census_source,
            before_served,
            cand,
            prev,
            ttimes,
            plans,
            n_exam,
            n_unchanged,
            taken_by_table,
            slot_a: first_table_slot,
            certified_keys,
            bits,
            resumed,
            trials,
            solved_lines,
            records_by_key,
            recipe_files,
            jobs_log,
            guard_cpu_s,
            guard_wall_s,
            bound,
        })
    }

    /// The guard jobs of round `round` (the (rule, step, keys) still needed); `None` when nothing is needed any more
    /// (an empty list: every needed trial came from the checkpoint).
    pub fn round_jobs(
        &mut self,
        cfg: &ShipRunConfig,
        round: usize,
    ) -> Result<Option<Vec<Job>>, String> {
        let say = |m: String| {
            if cfg.verbose {
                eprintln!("[shipecon {} {}] {m}", cfg.node, now());
            }
        };
        let ShipState {
            p,
            census,
            set0,
            plans,
            trials,
            resumed,
            recipe_files,
            bound,
            ..
        } = self;
        let p: &ShipPolicy = p;
        let set_a: &ImportedSet = set0;
        let bound: &Value = bound;
        // the (rule, step, keys) still needed
        let mut need: BTreeMap<(usize, String), Vec<Vec<i32>>> = BTreeMap::new();
        for (&i, r) in plans.iter() {
            if *r == Reuse::Unchanged {
                continue;
            }
            for (s, k) in ship::missing_trials(p, &census.rules[i], &trials[&i]) {
                need.entry((i, s)).or_default().push(k);
            }
        }
        if need.is_empty() {
            return Ok(None);
        }
        let mut queue: Vec<Job> = vec![];
        let mut n_resumed = 0usize;
        for ((i, s), keys) in &need {
            let rule = &census.rules[*i].rule;
            for (rid, doc, rkeys) in recipes_for(p, set_a, rule, s, keys)? {
                let mut text = rustred_campaign_core::schema::canon::canonical_json(&doc, false)?;
                text.push('\n');
                let sha = sha256_hex(text.as_bytes());
                let path = cfg
                    .out_dir
                    .join("recipes")
                    .join(format!("{}.json", crate::closeloop::ids::safe(&rid)));
                write(&path, text.as_bytes())?;
                recipe_files.insert(rid.clone(), (path.clone(), sha.clone(), text.len() as u64));
                // the checkpoint of an interrupted run: same recipe bytes, reusable verdict
                let rsha = &census.rules[*i].rule.record_sha256;
                let mut rkeys = rkeys;
                rkeys.retain(|k| {
                    let Some(t) = resumed.get(&(rsha.clone(), s.clone(), k.clone())) else {
                        return true;
                    };
                    if t.recipe_sha256 == sha && (t.deterministic() || t.bound == *bound) {
                        let tr = trials.entry(*i).or_default();
                        tr.retain(|x| !(x.key == t.key && x.step == t.step));
                        tr.push(t.clone());
                        n_resumed += 1;
                        false
                    } else {
                        true
                    }
                });
                for (c, chunk) in rkeys.chunks(p.recipe.max_keys_per_job).enumerate() {
                    queue.push(Job {
                        id: format!("r{round}-{}-c{c}", crate::closeloop::ids::safe(&rid)),
                        rule: *i,
                        step: s.clone(),
                        recipe_id: rid.clone(),
                        recipe_path: path.clone(),
                        recipe_sha256: sha.clone(),
                        keys: chunk.to_vec(),
                    });
                }
            }
        }
        say(format!(
            "round {round}: {} jobs, {} keys ({n_resumed} trials resumed from the checkpoint)",
            queue.len(),
            queue.iter().map(|j| j.keys.len()).sum::<usize>()
        ));
        Ok(Some(queue))
    }

    /// One settled guard job: the closeloop accept chain on its sealed results, its trials, the checkpoint line.
    pub fn settle(
        &mut self,
        cfg: &ShipRunConfig,
        round: usize,
        job: Job,
        result: JobResult,
    ) -> Result<Value, String> {
        let say = |m: String| {
            if cfg.verbose {
                eprintln!("[shipecon {} {}] {m}", cfg.node, now());
            }
        };
        let ShipState {
            p,
            census,
            set0,
            trials,
            slot_a,
            certified_keys,
            bits,
            solved_lines,
            records_by_key,
            jobs_log,
            guard_cpu_s,
            guard_wall_s,
            bound,
            ..
        } = self;
        let p: &ShipPolicy = p;
        let set_a: &mut ImportedSet = set0;
        let bound: &Value = bound;
        let slot_a: &DispatchSlot = slot_a;
        let mut new_trials: Vec<KeyTrial> = vec![];
        let mut job_rec = json!({"job": job.id, "rule": census.rules[job.rule].rule.id(), "step": job.step,
            "recipe": job.recipe_id, "keys": job.keys.len()});
        match result {
            Err(e) => {
                job_rec["error"] = json!(e);
                for k in &job.keys {
                    new_trials.push(failed_trial(&job, k, "REFUSED", &e, bound));
                }
            }
            Ok((results, lines, b3, run_dir, cost)) => {
                job_rec["run_dir"] = json!(run_dir);
                job_rec["cost"] =
                    json!({"cpu_s": cost.cpu_s, "wall_s": cost.wall_s, "vmhwm_kb": cost.vmhwm_kb});
                *guard_cpu_s += cost.cpu_s.unwrap_or(0.0);
                *guard_wall_s += cost.wall_s.unwrap_or(0.0);
                if lines.is_empty() {
                    job_rec["error"] = json!(results);
                    for k in &job.keys {
                        new_trials.push(failed_trial(&job, k, "REFUSED", &results, bound));
                    }
                } else {
                    let batch = BatchFile {
                        schema: BATCH_SCHEMA.into(),
                        node: cfg.node.clone(),
                        item: job.id.clone(),
                        round: round as u32,
                        source: job.recipe_id.clone(),
                        recipe_path: job.recipe_path.display().to_string(),
                        recipe_sha256: job.recipe_sha256.clone(),
                        recipe_blake3: crate::atomic::blake3_file(&job.recipe_path)
                            .map(|x| x.1)
                            .unwrap_or_default(),
                        engine_digest: None,
                        relation_rule: p.recipe.relation_rule.clone(),
                        keys: job.keys.clone(),
                    };
                    let raw = raw_records(&results)?;
                    match accept_point(
                        set_a,
                        None,
                        &batch,
                        slot_a,
                        &lines,
                        bits,
                        (&results, &b3),
                        &run_dir,
                        None,
                    ) {
                        Err(e) => {
                            job_rec["error"] = json!(format!("accept chain: {e}"));
                            for k in &job.keys {
                                new_trials.push(failed_trial(&job, k, "REFUSED", &e, bound));
                            }
                        }
                        Ok(dec) => {
                            solved_lines.extend(dec.lines.iter().cloned());
                            let lines_by_key: BTreeMap<Vec<i32>, String> = dec
                                .lines
                                .iter()
                                .filter_map(|l| {
                                    let v: Value = serde_json::from_str(l).ok()?;
                                    let k: Vec<i32> =
                                        serde_json::from_value(v["key"].clone()).ok()?;
                                    Some((k, l.clone()))
                                })
                                .collect();
                            let cert: BTreeSet<&Vec<i32>> = dec.certified.iter().collect();
                            let failed: BTreeMap<&Vec<i32>, &String> =
                                dec.failed.iter().map(|(k, w)| (k, w)).collect();
                            let retry: BTreeMap<&Vec<i32>, &String> =
                                dec.retry.iter().map(|(k, w)| (k, w)).collect();
                            for k in &job.keys {
                                let r = raw.get(k);
                                let verdict = r
                                    .and_then(|r| r["verdict"].as_str())
                                    .unwrap_or("MISSING")
                                    .to_string();
                                let body = r.map(entry_body);
                                let entry_bytes = body.as_ref().map(|b| {
                                    rustred_campaign_core::schema::canon::canonical_json(b, true)
                                        .map(|t| t.len() as u64 + 600)
                                        .unwrap_or(0)
                                });
                                if cert.contains(k)
                                    && let Some(rv) = r
                                {
                                    records_by_key.insert(k.clone(), rv.clone());
                                }
                                new_trials.push(KeyTrial {
                                    key: k.clone(),
                                    step: job.step.clone(),
                                    recipe_id: job.recipe_id.clone(),
                                    recipe_sha256: job.recipe_sha256.clone(),
                                    recipe_digest: r
                                        .and_then(|r| r["recipe_digest"].as_str())
                                        .map(str::to_string),
                                    verdict: verdict.clone(),
                                    certified: cert.contains(k),
                                    relation_digest: r
                                        .and_then(|r| r["relation_digest"].as_str())
                                        .map(str::to_string),
                                    seconds: r.and_then(|r| r["seconds"].as_f64()),
                                    rhs_terms: r.and_then(|r| r["rhs_count"].as_u64()),
                                    entry_bytes,
                                    pole_order_d4: r
                                        .and_then(|r| r["pole_order_d4"].as_u64())
                                        .map(|x| x as u32),
                                    rhs_outside_certified: r.map(|r| {
                                        r["rhs"]
                                            .as_array()
                                            .into_iter()
                                            .flatten()
                                            .filter(|t| t[2] == "top")
                                            .filter_map(|t| {
                                                t[0].as_str().and_then(
                                                    rustred_campaign_core::assemble::parse_key_text,
                                                )
                                            })
                                            .filter(|k| !certified_keys.contains(k))
                                            .count() as u64
                                    }),
                                    why: if cert.contains(k) {
                                        None
                                    } else {
                                        Some(
                                            failed
                                                .get(k)
                                                .or(retry.get(k))
                                                .map(|w| w.to_string())
                                                .unwrap_or_else(|| verdict.clone()),
                                        )
                                    },
                                    bound: bound.clone(),
                                    solved_line: lines_by_key.get(k).cloned(),
                                    results: Some(results.clone()),
                                });
                            }
                            job_rec["certified"] = json!(dec.certified.len());
                            job_rec["failed"] = json!(dec.failed.len() + dec.retry.len());
                        }
                    }
                }
            }
        }
        say(format!(
            "settled {}: {}",
            job.id,
            job_rec["error"].as_str().map_or_else(
                || format!("certified {} of {}", job_rec["certified"], job.keys.len()),
                |e| format!("refused: {e}")
            )
        ));
        jobs_log.push(job_rec);
        // the per-job checkpoint (resume after a stop / crash: `resume_trials`)
        {
            use std::io::Write as _;
            let sha = &census.rules[job.rule].rule.record_sha256;
            // procfix7 round 19 (landing, int8b-gate review (ii) shipecon.rs:1342)): a checkpoint that cannot be written is NAMED (it was dropped:
            // on resume those trials run again, nothing else depends on it)
            let mut buf = String::new();
            for t in &new_trials {
                buf.push_str(&json!({"record_sha256": sha, "trial": t}).to_string());
                buf.push('\n');
            }
            let cp = cfg.out_dir.join("trials.jsonl");
            let written = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&cp)
                .and_then(|mut f| f.write_all(buf.as_bytes()))
                .map_err(|e| format!("{}: {e}", cp.display()));
            if let Err(e) = written {
                say(format!(
                    "checkpoint not written ({e}): on resume the trials of {} run again",
                    job.id
                ));
            }
        }
        let t = trials.entry(job.rule).or_default();
        for nt in new_trials {
            t.retain(|x| !(x.key == nt.key && x.step == nt.step));
            t.push(nt);
        }
        Ok(jobs_log.last().cloned().unwrap_or(Value::Null))
    }

    /// Decisions, the closing check of the shipped spec (with the served-key-set check and the dead-entry rule), the
    /// records, the report and (apply mode) the artifact.
    pub fn finish(self, cfg: &ShipRunConfig) -> Result<Value, String> {
        let say = |m: String| {
            if cfg.verbose {
                eprintln!("[shipecon {} {}] {m}", cfg.node, now());
            }
        };
        let ShipState {
            p,
            t0,
            isp,
            set0,
            load_s,
            census,
            before,
            census_source,
            before_served,
            cand,
            prev,
            ttimes,
            plans,
            n_exam,
            n_unchanged,
            taken_by_table,
            bits,
            trials,
            mut solved_lines,
            mut records_by_key,
            mut recipe_files,
            jobs_log,
            guard_cpu_s,
            guard_wall_s,
            ..
        } = self;
        let p: &ShipPolicy = &p;
        // reused trials (previous records / checkpoint): their answer lines and engine records, from what they recorded
        let fresh: BTreeSet<String> = solved_lines.iter().cloned().collect();
        let mut raw_cache: BTreeMap<String, BTreeMap<Vec<i32>, Value>> = BTreeMap::new();
        for ts in trials.values() {
            for t in ts {
                if let Some(l) = &t.solved_line
                    && !fresh.contains(l)
                {
                    solved_lines.push(l.clone());
                }
                if t.certified
                    && !records_by_key.contains_key(&t.key)
                    && let Some(rp) = &t.results
                {
                    if !raw_cache.contains_key(rp) {
                        raw_cache.insert(rp.clone(), raw_records(rp)?);
                    }
                    let Some(m) = raw_cache.get(rp) else {
                        continue;
                    };
                    if let Some(r) = m.get(&t.key) {
                        records_by_key.insert(t.key.clone(), r.clone());
                    }
                }
            }
        }
        if let Some(ps) = &cfg.previous_solved {
            let have: BTreeSet<String> = solved_lines.iter().cloned().collect();
            let text = std::fs::read_to_string(ps).map_err(|e| format!("{}: {e}", ps.display()))?;
            for l in text.lines().filter(|l| !l.trim().is_empty()) {
                if !have.contains(l) {
                    solved_lines.push(l.to_string());
                }
            }
        }
        if let Some(pw) = &cfg.previous_work {
            // key -> the certified relation digest of its trial (a key may have SOLVED records of several steps)
            let want: BTreeMap<Vec<i32>, String> = trials
                .values()
                .flatten()
                .filter(|t| t.certified && !records_by_key.contains_key(&t.key))
                .filter_map(|t| t.relation_digest.clone().map(|d| (t.key.clone(), d)))
                .collect();
            if !want.is_empty() {
                let mut stack = vec![pw.clone()];
                while let Some(d) = stack.pop() {
                    // procfix7 round 19 (landing, int8b-gate review (ii) shipecon.rs:1442-1447) / rows 8-9): a failed listing is never "no records"; the
                    // dirent type decides (no extra stat; a symlinked directory is no longer descended)
                    let entries = match crate::fsx::list("closeloop.input", &d) {
                        crate::fsx::Listing::Complete(v) => v,
                        crate::fsx::Listing::Absent => continue,
                        crate::fsx::Listing::Failed(e) => {
                            return Err(format!("{}: listing failed: {e}", d.display()));
                        }
                    };
                    for e in entries {
                        let p = e.path;
                        if e.file_type.is_dir() {
                            stack.push(p);
                        } else if p.file_name().is_some_and(|n| n == "results.jsonl") {
                            for (k, r) in raw_records(&p.display().to_string())? {
                                if want
                                    .get(&k)
                                    .is_some_and(|d| r["relation_digest"] == d.as_str())
                                {
                                    records_by_key.entry(k).or_insert(r);
                                }
                            }
                        }
                    }
                }
            }
        }
        // recipe files of reused trials, before the decisions (their bytes enter the recipe kind): this run's own
        // `recipes/` (a resumed supervised campaign), else the previous run's, each verified by its recorded sha256
        let prev_dir = cfg.previous.as_ref().and_then(|p| p.parent());
        for t in trials.values().flatten().filter(|t| t.certified) {
            if recipe_files.contains_key(&t.recipe_id) {
                continue;
            }
            let name = format!("{}.json", crate::closeloop::ids::safe(&t.recipe_id));
            let local = cfg.out_dir.join("recipes").join(&name);
            // procfix7 round 19 (landing, int8b-gate review (ii) shipecon.rs:1472, 1481-1482)): a recipe file that cannot be READ is an error (it fell
            // through to the next copy, then to "no file"); not there or another sha = the next copy
            let read_recipe = |p: &Path| -> Result<Option<Vec<u8>>, String> {
                match crate::fsx::read("closeloop.input", p) {
                    crate::fsx::Read::Content(b) => Ok(Some(b)),
                    crate::fsx::Read::Absent => Ok(None),
                    crate::fsx::Read::Failed(e) => Err(format!("{}: {e}", p.display())),
                }
            };
            if let Some(b) = read_recipe(&local)?
                && sha256_hex(&b) == t.recipe_sha256
            {
                recipe_files.insert(
                    t.recipe_id.clone(),
                    (local, t.recipe_sha256.clone(), b.len() as u64),
                );
                continue;
            }
            let prev_copy = match prev_dir {
                Some(pd) => read_recipe(&pd.join("recipes").join(&name))?,
                None => None,
            };
            if let Some(b) = prev_copy
                && sha256_hex(&b) == t.recipe_sha256
            {
                write(&local, &b)?;
                recipe_files.insert(
                    t.recipe_id.clone(),
                    (local, t.recipe_sha256.clone(), b.len() as u64),
                );
            }
        }
        let guard_s = t0.elapsed().as_secs_f64() - load_s;
        // decisions
        let mut decisions: Vec<RuleDecision> = vec![];
        let mut guards: BTreeMap<usize, ship::GuardRecord> = BTreeMap::new();
        let mut table_time_of: BTreeMap<usize, TableTime> = BTreeMap::new();
        for (i, c) in cand.iter().enumerate() {
            let svc = &census.rules[i];
            match c {
                Candidacy::Examine => {
                    if plans.get(&i) == Some(&Reuse::Unchanged)
                        && let Some(pr) = prev.get(&svc.rule.record_sha256)
                    {
                        decisions.push(pr.decision.clone());
                        if let Some(g) = &pr.guard {
                            guards.insert(i, g.clone());
                        }
                        if let Some(tt) = &pr.table_time {
                            table_time_of.insert(i, tt.clone());
                        }
                        continue;
                    }
                    let tt = ttimes
                        .get(&svc.rule.record_sha256)
                        .cloned()
                        .unwrap_or_else(|| TableTime::estimate(&p.cost, svc.bytes, svc.monomials));
                    let g = ship::guard(p, svc, trials.get(&i).map(Vec::as_slice).unwrap_or(&[]));
                    let rb: BTreeMap<String, u64> =
                        recipe_files.iter().map(|(k, v)| (k.clone(), v.2)).collect();
                    decisions.push(ship::decide(p, svc, &g, &tt, &rb, &taken_by_table));
                    guards.insert(i, g);
                    table_time_of.insert(i, tt);
                }
                other => decisions.push(ship::decide_unexamined(svc, other)),
            }
        }
        // the closing check of the shipped spec, reverting the rules behind a failure
        let solved_path = cfg.out_dir.join("ship/solved.jsonl");
        let mut solved_text = String::new();
        for l in &solved_lines {
            solved_text.push_str(l);
            solved_text.push('\n');
        }
        write(&solved_path, solved_text.as_bytes())?;
        let solved_ref = FileRef {
            path: solved_path.display().to_string(),
            sha256: Some(sha256_hex(solved_text.as_bytes())),
            manifest: None,
            family: set0.family.clone(),
        };
        let mut check_log: Vec<Value> = vec![];
        let mut closing: Option<ClosingCheck> = None;
        let mut final_spec: Option<TableSetSpec> = None;
        let mut probe: Option<Value> = None;
        let mut services = Value::Null;
        let local = cfg.check_mode == "local";
        if local && !p.coverage.require_rhs_in_certified_set {
            return Err("check_mode local needs coverage.require_rhs_in_certified_set".into());
        }
        if local && let Some(fp) = &cfg.final_probe {
            let text = std::fs::read_to_string(fp).map_err(|e| format!("{}: {e}", fp.display()))?;
            let mut taken: BTreeSet<Vec<i32>> = BTreeSet::new();
            for l in text.lines().filter(|l| !l.trim().is_empty()) {
                let v: Value =
                    serde_json::from_str(l).map_err(|e| format!("{}: {e}", fp.display()))?;
                if !v["position"].is_null()
                    && let Ok(k) = serde_json::from_value::<Vec<i32>>(v["key"].clone())
                {
                    taken.insert(k);
                }
            }
            let n: usize = decisions
                .iter_mut()
                .map(|d| ship::drop_dead_entries(d, &taken))
                .sum();
            check_log.push(
                json!({"mode": "local", "final_probe": fp.display().to_string(),
                "dead_entries_dropped": n}),
            );
        }
        if local {
            // every migrated key: served by its certified relation (the accept chain: exact proof, regularity, strict
            // descent under the full comparator) whose top right-hand side lies in the certified set (decide refused the
            // others): every reached key stays a certified key with its old service or its new relation, every new edge
            // descends (acyclic); the closure can only shrink; terminals are a subset (reachability not recomputed)
            // a key a LATER table rule takes stays served, but that rule's right-hand side is outside the local proof:
            // its rule is reverted (conservative, per rule); `stored.precedence = over_table` has no such keys
            let mut later_reverted: Vec<String> = vec![];
            for d in decisions.iter_mut().filter(|d| d.outcome.migrates()) {
                let n = d
                    .keys
                    .iter()
                    .filter(|k| k.form == "later_table_rule")
                    .count();
                if n > 0 {
                    later_reverted.push(format!("{} ({n} keys)", d.rule.id()));
                    ship::revert(
                        d,
                        &format!(
                            "local check: {n} keys taken by a later table rule whose right-hand side the local proof does \
                             not cover (full check, or stored.precedence over_table)"
                        ),
                    );
                }
            }
            let migrated: Vec<&RuleDecision> =
                decisions.iter().filter(|d| d.outcome.migrates()).collect();
            let unserved: Vec<Vec<i32>> = migrated
                .iter()
                .flat_map(|d| d.keys.iter())
                .filter(|k| !matches!(k.form.as_str(), "recipe" | "stored"))
                .map(|k| k.key.clone())
                .collect();
            let cc = ClosingCheck {
                closed: before.blocking.is_empty() && before.open == 0,
                blocking: before.blocking.clone(),
                same_start: true,
                same_terminals: true,
                terminals_before: before.terminals.clone(),
                terminals_after: before.terminals.clone(),
                start_keys: before.start.len() as u64,
                closure_before: before.keys.len() as u64,
                closure_after: before.keys.len() as u64,
                new_keys: 0,
                dropped_keys: 0,
                open_before: before.open,
                open_after: before.open,
                new_open_keys: 0,
                new_blocking: vec![],
                holds: unserved.is_empty(),
            };
            check_log.push(json!({"mode": "local", "migrating": migrated.len(), "unserved_migrated_keys": unserved,
                "reverted_later_rule": later_reverted,
                "note": "local proof (the material is a selection of the node's rules): closure membership and service of \
                         every key; NOT computed: dropped keys, terminal reachability"}));
            if !cc.holds {
                for d in decisions.iter_mut().filter(|d| d.outcome.migrates()) {
                    ship::revert(
                        d,
                        "local check: a migrated key without a certified relation",
                    );
                }
            }
            closing = Some(cc);
        }
        for it in 0..=p.check.max_iterations {
            if local {
                break;
            }
            let (recipes, assign) = shipped_recipes(cfg, &decisions, &recipe_files)?;
            let mut sspec = ship::shipped_spec(
                &isp,
                &decisions,
                &recipes,
                std::slice::from_ref(&solved_ref),
                p.stored.precedence,
            )?;
            sspec.evidence.expected_recipe_binary = Some(bits.binary_sha256.clone());
            let sp = cfg.out_dir.join(format!("ship/spec.iter{it}.json"));
            write(
                &sp,
                &serde_json::to_vec_pretty(&ship::spec_json(&sspec)).map_err(|e| e.to_string())?,
            )?;
            let tc = Instant::now();
            let set_s = load(&sspec, &cfg.import_base).map_err(|e| format!("shipped load: {e}"))?;
            let mut ev = Evidence::from_spec(&set_s, &cfg.import_base)
                .map_err(|e| format!("shipped evidence: {e}"))?;
            ev.accept_ledger_verified_files();
            let res = check(
                &set_s,
                &ev,
                &CheckOptions {
                    threads: cfg.check_threads.max(1),
                    tier: None,
                    max_rounds: None,
                },
            )
            .map_err(|e| format!("shipped check: {e}"))?;
            let after = view_of(&set_s, &res)?;
            let cc = ClosingCheck::compare(&before, &after);
            say(format!(
                "closing check {it}: closed {} same start {} same terminals {} open {} new keys {} dropped {} ({:.1} s)",
                cc.closed,
                cc.same_start,
                cc.same_terminals,
                cc.open_after,
                cc.new_keys,
                cc.dropped_keys,
                tc.elapsed().as_secs_f64()
            ));
            let mut entry = json!({"iteration": it, "check": cc, "recipes": recipes.len(), "assignments": assign,
                "migrating": decisions.iter().filter(|d| d.outcome.migrates()).count(),
                "seconds": tc.elapsed().as_secs_f64()});
            if cc.holds {
                // COVERAGE NEVER REDUCED, key by key: the shipped spec with EVERY key the original serves in its start set
                let sc = served_set_check(cfg, &sspec, &before_served, it)?;
                let rows_main = all_rows(&set_s, &res)?;
                let reached: BTreeSet<&Vec<i32>> = rows_main.iter().map(|r| &r.key).collect();
                let start: BTreeSet<&Vec<i32>> = before.start.iter().collect();
                let dropped: Vec<&Vec<i32>> = before
                    .keys
                    .iter()
                    .filter(|k| !reached.contains(k))
                    .collect();
                let lost: BTreeSet<&Vec<i32>> = sc.lost.iter().collect();
                entry["served_set"] = json!({"original": sc.original, "kept": sc.kept, "lost": sc.lost.len(),
                    "lower": sc.lower.len(), "holds": sc.holds,
                    "lost_examples": sc.lost.iter().take(50).collect::<Vec<_>>(),
                    "lower_examples": sc.lower.iter().take(50).collect::<Vec<_>>(),
                    "dropped_from_closure": dropped.len(),
                    "dropped_in_start": dropped.iter().filter(|k| start.contains(*k)).count(),
                    "dropped_still_served": dropped.iter().filter(|k| !lost.contains(*k)).count(),
                    "dropped_examples": dropped.iter().take(50).collect::<Vec<_>>()});
                say(format!(
                    "served-key-set check {it}: {} of {} original keys served (lost {}, lower evidence {}); dropped from the \
                     closure {} (in the start set {})",
                    sc.kept,
                    sc.original,
                    sc.lost.len(),
                    sc.lower.len(),
                    dropped.len(),
                    dropped.iter().filter(|k| start.contains(*k)).count()
                ));
                if sc.holds {
                    // NEVER SHIP A DEAD ENTRY: a migrated key the shipped dispatch still gives to a table rule
                    let migrated: BTreeSet<&Vec<i32>> = decisions
                        .iter()
                        .filter(|d| d.outcome.migrates())
                        .flat_map(|d| d.keys.iter().map(|k| &k.key))
                        .collect();
                    let dead: BTreeSet<Vec<i32>> = rows_main
                        .iter()
                        .filter(|r| migrated.contains(&r.key))
                        .filter(|r| matches!(r.server, Some(Server::Rule { .. })))
                        .map(|r| r.key.clone())
                        .collect();
                    let n_dead: usize = decisions
                        .iter_mut()
                        .map(|d| ship::drop_dead_entries(d, &dead))
                        .sum();
                    entry["dead_entries_dropped"] = json!(n_dead);
                    if n_dead > 0 {
                        // the relation sets of dead keys never served (the table took them): the spec without them
                        // dispatches every key as checked
                        let (recipes2, _) = shipped_recipes(cfg, &decisions, &recipe_files)?;
                        sspec = ship::shipped_spec(
                            &isp,
                            &decisions,
                            &recipes2,
                            std::slice::from_ref(&solved_ref),
                            p.stored.precedence,
                        )?;
                        sspec.evidence.expected_recipe_binary = Some(bits.binary_sha256.clone());
                    }
                    // services per kind over the shipped closure (the manifest's `services`)
                    let (recipes_now, _) = shipped_recipes(cfg, &decisions, &recipe_files)?;
                    let form_of: BTreeMap<&str, &str> = recipes_now
                        .iter()
                        .map(|r| (r.id.as_str(), r.form.as_str()))
                        .collect();
                    let mut sv: BTreeMap<&str, u64> = BTreeMap::new();
                    for r in &rows_main {
                        let k = match (&r.status, &r.server) {
                            (RowStatus::Served, Some(Server::Rule { .. })) => "table",
                            (RowStatus::Served, Some(Server::Relation { set, .. })) => {
                                match form_of.get(set.as_str()) {
                                    Some(&"stored") => "entry",
                                    Some(_) => "recipe_record",
                                    None => "other_relation",
                                }
                            }
                            (RowStatus::Served, _) => "credit",
                            (RowStatus::Terminal, _) => "terminal",
                            (RowStatus::Open, _) => "open",
                        };
                        *sv.entry(k).or_default() += 1;
                    }
                    services = json!(sv);
                    probe = Some(engine_probe(cfg, &set_s, &res, &decisions)?);
                    check_log.push(entry);
                    closing = Some(cc);
                    final_spec = Some(sspec);
                    break;
                }
                // a coverage loss: revert the rules that served the lost / lower keys
                let bad: BTreeSet<Vec<i32>> = sc
                    .lost
                    .iter()
                    .cloned()
                    .chain(sc.lower.iter().map(|x| x.0.clone()))
                    .collect();
                let mut culprits: Vec<usize> = vec![];
                for (di, d) in decisions.iter().enumerate() {
                    if d.outcome.migrates() && d.keys.iter().any(|k| bad.contains(&k.key)) {
                        culprits.push(di);
                    }
                }
                if culprits.is_empty() || it == p.check.max_iterations {
                    entry["reverted"] =
                        json!("all (served-key-set loss not attributable, or last iteration)");
                    for d in decisions.iter_mut().filter(|d| d.outcome.migrates()) {
                        ship::revert(
                            d,
                            &format!("served-key-set check {it}: {} keys lost", bad.len()),
                        );
                    }
                } else {
                    entry["reverted"] = json!(
                        culprits
                            .iter()
                            .map(|&di| decisions[di].rule.id())
                            .collect::<Vec<_>>()
                    );
                    for &di in &culprits {
                        ship::revert(
                            &mut decisions[di],
                            "served-key-set check: a key the original statement serves is not served (or at a lower \
                             evidence level) by the shipped artifact",
                        );
                    }
                }
                check_log.push(entry);
                closing = Some(cc);
                continue;
            }
            // culprits: open keys / new terminals; the rules that served them before, or whose relations reach them
            let rows = all_rows(&set_s, &res)?;
            // NEW failures only: a key open in the original is not one; a new terminal is
            let was_open: BTreeSet<&Vec<i32>> = before.open_keys.iter().collect();
            let bad_rows: Vec<&rustred_campaign_core::import::KeyRow> = rows
                .iter()
                .filter(|r| {
                    (r.status == RowStatus::Open && !was_open.contains(&r.key))
                        || (r.status == RowStatus::Terminal && !before.terminals.contains(&r.key))
                })
                .collect();
            entry["bad_examples"] = json!(
                bad_rows
                    .iter()
                    .take(50)
                    .map(
                        |r| json!({"key": r.key, "status": format!("{:?}", r.status),
                        "open_status": r.open_status, "open_reason": r.open_reason})
                    )
                    .collect::<Vec<_>>()
            );
            let bad: BTreeSet<Vec<i32>> = bad_rows.iter().map(|r| r.key.clone()).collect();
            let mut culprits: BTreeSet<usize> = BTreeSet::new();
            for (di, d) in decisions.iter().enumerate() {
                if !d.outcome.migrates() {
                    continue;
                }
                let reaches = d.keys.iter().any(|k| {
                    bad.contains(&k.key)
                        || records_by_key.get(&k.key).is_some_and(|r| {
                            r["rhs"].as_array().into_iter().flatten().any(|t| {
                                t[0].as_str()
                                    .and_then(rustred_campaign_core::assemble::parse_key_text)
                                    .is_some_and(|rk| bad.contains(&rk))
                            })
                        })
                });
                if reaches {
                    culprits.insert(di);
                }
            }
            if culprits.is_empty() || it == p.check.max_iterations {
                // nothing attributable: revert every migration (the shipped spec = the original material)
                entry["reverted"] = json!("all (no attributable culprit or last iteration)");
                for d in decisions.iter_mut().filter(|d| d.outcome.migrates()) {
                    ship::revert(
                        d,
                        &format!("closing check iteration {it} failed: {:?}", cc.blocking),
                    );
                }
            } else {
                entry["reverted"] = json!(
                    culprits
                        .iter()
                        .map(|&di| decisions[di].rule.id())
                        .collect::<Vec<_>>()
                );
                for &di in &culprits {
                    ship::revert(
                        &mut decisions[di],
                        &format!(
                            "its keys or relations reach {} open / new-terminal keys",
                            bad.len()
                        ),
                    );
                }
            }
            entry["bad_keys"] = json!(bad.len());
            check_log.push(entry);
            closing = Some(cc);
        }
        // records, report, artifact
        let mut records: Vec<EconomyRecord> = vec![];
        for (i, c) in cand.iter().enumerate() {
            if !matches!(c, Candidacy::Examine | Candidacy::KeptAside) {
                continue;
            }
            let svc = &census.rules[i];
            let d = decisions[i].clone();
            let g = guards.get(&i).cloned();
            let info = set0
                .find_sha(&svc.rule.record_sha256)
                .map(|(t, r)| set0.rules[t][r].clone());
            records.push(EconomyRecord {
                schema: ECONOMY_RECORD_SCHEMA.into(),
                kind: RECORD_KIND.into(),
                node: cfg.node.clone(),
                subject: svc.rule.clone(),
                inputs_digest: ship::inputs_digest(svc),
                decision_digest: p.decision_digest(),
                policy_digest: p.digest(),
                trials: trials.get(&i).cloned().unwrap_or_default(),
                table_time: table_time_of.get(&i).cloned(),
                not_tried: ship::not_tried(p, &d, g.as_ref()),
                decision: d,
                guard: g,
                revert: json!({"table_path": info.as_ref().map(|x| x.path.clone()), "table_sha256": svc.rule.table_sha256,
                    "offset": info.as_ref().map(|x| x.offset), "bytes": svc.bytes,
                    "how": "remove the rule's entry from the shipped spec's `removed` list (the original table is kept)"}),
            });
        }
        write(&cfg.out_dir.join("records.jsonl"), &jsonl(&records))?;
        let guard_list: Vec<&ship::GuardRecord> = guards.values().collect();
        write(&cfg.out_dir.join("guards.jsonl"), &jsonl(&guard_list))?;
        write(&cfg.out_dir.join("jobs.jsonl"), &jsonl(&jobs_log))?;
        let totals = ship::totals(&census, &decisions);
        let applied = if p.mode == ShipMode::Apply && closing.as_ref().is_some_and(|c| c.holds) {
            Some(write_artifact(
                cfg,
                &census,
                &decisions,
                final_spec.as_ref(),
                &records_by_key,
                &set0,
                &json!({"services": services, "certified_keys": before.keys.len(), "source": census_source}),
            )?)
        } else {
            None
        };
        let per_rule: Vec<Value> = decisions
            .iter()
            .filter(|d| d.outcome != Outcome::NotExamined)
            .map(|d| serde_json::to_value(d).unwrap_or(Value::Null))
            .collect();
        let not_examined: BTreeMap<String, u64> = decisions
            .iter()
            .filter(|d| d.outcome == Outcome::NotExamined)
            .fold(BTreeMap::new(), |mut m, d| {
                // the reason CLASS: the policy field it names
                let w = [
                    "candidates.min_rule_bytes",
                    "candidates.max_keys_per_rule",
                    "candidates.max_guard_keys",
                    "candidates.max_rules",
                    "coverage.target",
                    "kept_aside",
                ]
                .into_iter()
                .find(|f| d.why.contains(f))
                .unwrap_or("other")
                .to_string();
                *m.entry(w).or_default() += 1;
                m
            });
        let v = json!({
            "schema": REPORT_SCHEMA,
            "node": cfg.node,
            "mode": p.mode,
            "policy": p.to_value(),
            "policy_digest": p.digest(),
            "decision_digest": p.decision_digest(),
            "import_spec": {"path": cfg.import_spec.display().to_string(), "digest": isp.digest()},
            "census_source": census_source,
            "census": census.totals(),
            "before": {"keys": before.keys.len(), "start": before.start.len(), "terminals": before.terminals, "open": before.open},
            "examined": n_exam,
            "unchanged_since_previous": n_unchanged,
            "previous": cfg.previous.as_ref().map(|x| x.display().to_string()),
            "binary": {"path": bits.binary_path, "sha256": bits.binary_sha256, "blake3": bits.binary_blake3},
            "table_times": cfg.table_times.as_ref().map(|x| x.display().to_string()),
            "totals": totals,
            "closing_check": closing,
            "closing_iterations": check_log,
            "decisions": per_rule,
            "not_examined_by_reason": not_examined,
            "engine_probe": probe,
            "applied": applied,
            "timing": {"load_s": load_s, "guard_s": guard_s, "guard_jobs_cpu_s": guard_cpu_s,
                       "guard_jobs_wall_s": guard_wall_s, "wall_s": t0.elapsed().as_secs_f64(),
                       "note": "wall times on a shared host, unpinned (not a benchmark)"},
        });
        write(
            &cfg.out_dir.join("report.json"),
            &serde_json::to_vec_pretty(&v).map_err(|e| e.to_string())?,
        )?;
        Ok(v)
    }
}

/// Run the ship economy to its end (the direct driver); returns the report (also written to `out_dir/report.json`).
pub fn run_ship(cfg: &ShipRunConfig) -> Result<Value, String> {
    let mut st = ShipState::prepare(cfg)?;
    let sampler = Arc::new(
        Sampler::spawn(SamplerConfig {
            interval: Duration::from_millis(500),
            collect_foreign_pinning: false,
            // procfix7 round 7 field (merge): no effect, pinning is not collected
            foreign_pin_max_cpus: usize::MAX,
        })
        .map_err(|e| format!("sampler: {e}"))?,
    );
    for round in 0..cfg.policy.recipe.steps.len() {
        let Some(queue) = st.round_jobs(cfg, round)? else {
            break;
        };
        let (tx, rx) = mpsc::channel::<Done>();
        let mut in_flight = 0usize;
        let mut queue = queue.into_iter();
        loop {
            while in_flight < cfg.parallel.max(1) {
                let Some(j) = queue.next() else { break };
                launch(cfg, j, sampler.clone(), tx.clone())?;
                in_flight += 1;
            }
            if in_flight == 0 {
                break;
            }
            let d = rx.recv().map_err(|e| format!("worker channel: {e}"))?;
            in_flight -= 1;
            st.settle(cfg, round, d.job, d.result)?;
        }
    }
    st.finish(cfg)
}

/// The in-campaign hook (closing loop phase D, a CLOSED node): the report-mode proposal (default policy) next to the
/// statement: `<statement dir>/../shipecon/proposal.json` = census totals, candidacy, and per examined rule its bytes,
/// keys and the ESTIMATED economy (table time from the cost model, recipe seconds from the prior); the guard (engine
/// jobs) and the measured decision are the driver's ([`run_ship`]). Best effort: an error is written, never raised.
pub fn proposal_after_close(
    set: &ImportedSet,
    res: &rustred_campaign_core::import::CheckResult,
    sdir: &Path,
) {
    let dir = sdir
        .parent()
        .map(|d| d.join("shipecon"))
        .unwrap_or_else(|| sdir.join("shipecon"));
    let p = ShipPolicy::default();
    let out = (|| -> Result<Value, String> {
        let mut b = CensusBuilder::new(rules_of(set), p.candidates.max_keys_per_rule)?;
        for r in all_rows(set, res)? {
            let sha = match (&r.status, &r.server) {
                (RowStatus::Served, Some(Server::Rule { table, record })) => {
                    Some(set.rules[*table][*record].record_sha256.as_str())
                }
                _ => None,
            };
            b.add(&r.key, r.in_start, sha);
            if r.status == RowStatus::Open
                && let Some((t, rr)) = r.open_rule
            {
                b.add_open(&set.rules[t][rr].record_sha256);
            }
        }
        let census = b.finish();
        let cand = ship::candidacy(&p, &census);
        let mut examined = vec![];
        let mut kept = vec![];
        for (i, c) in cand.iter().enumerate() {
            let r = &census.rules[i];
            match c {
                Candidacy::Examine => {
                    let tt = TableTime::estimate(&p.cost, r.bytes, r.monomials);
                    examined.push(
                        json!({"rule": r.rule, "bytes": r.bytes, "in": r.served_in_start,
                        "out": r.served_outside_start,
                        "table_seconds_per_key_estimate": tt.per_key(&[], r.served()),
                        "recipe_seconds_per_key_prior": p.cost.recipe_solve_s}),
                    );
                }
                Candidacy::KeptAside => kept.push(json!({"rule": r.rule, "bytes": r.bytes})),
                Candidacy::NotExamined { .. } => {}
            }
        }
        Ok(
            json!({"schema": REPORT_SCHEMA, "mode": "proposal", "policy_digest": p.digest(),
            "census": census.totals(), "examined": examined, "kept_aside": kept,
            "next": "run the ship-economy driver (closeloop::shipecon::run_ship) for the guard, the measured decision \
                     and the closing check of the shipped spec"}),
        )
    })();
    let v =
        out.unwrap_or_else(|e| json!({"schema": REPORT_SCHEMA, "mode": "proposal", "error": e}));
    let _ = write(
        &dir.join("proposal.json"),
        &serde_json::to_vec_pretty(&v).unwrap_or_default(),
    );
}

/// The engine dispatch cross-check of the shipped spec: the probe table (header line, chart lines, the shipped rule
/// records VERBATIM in dispatch order: `rr_node first-match` positions are positions among them), the key sample and the
/// import's expected position per key (`null` = no table rule takes it: the key goes to the stored relation / recipe /
/// terminal); with `engine_first_match` given, the key-by-key comparison.
fn engine_probe(
    cfg: &ShipRunConfig,
    set: &ImportedSet,
    res: &rustred_campaign_core::import::CheckResult,
    decisions: &[RuleDecision],
) -> Result<Value, String> {
    let dir = cfg.out_dir.join("ship/probe");
    // positions of the shipped records in dispatch order
    let mut pos: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    let mut lines: Vec<Vec<u8>> = vec![];
    let mut header: Option<Vec<u8>> = None;
    let mut charts: Vec<Vec<u8>> = vec![];
    for &t in set.disp.priority() {
        let name = &set.disp.tables()[t].name;
        let Some(ti) = set.table_index(name) else {
            return Err(format!("probe: table {name} not in the set"));
        };
        for (r, info) in set.rules[ti].iter().enumerate() {
            let mut f =
                std::fs::File::open(&info.path).map_err(|e| format!("{}: {e}", info.path))?;
            use std::io::{Read, Seek, SeekFrom};
            if header.is_none() {
                let mut first = String::new();
                std::io::BufReader::new(
                    std::fs::File::open(&info.path).map_err(|e| e.to_string())?,
                )
                .read_line(&mut first)
                .map_err(|e| e.to_string())?;
                header = Some(first.trim_end().as_bytes().to_vec());
                for l in std::io::BufReader::new(
                    std::fs::File::open(&info.path).map_err(|e| e.to_string())?,
                )
                .lines()
                .map_while(Result::ok)
                .take_while(|l| !l.starts_with("{\"type\": \"rule\""))
                {
                    if l.starts_with("{\"type\": \"chart\"") {
                        charts.push(l.into_bytes());
                    }
                }
            }
            f.seek(SeekFrom::Start(info.offset))
                .map_err(|e| e.to_string())?;
            let mut b = vec![0u8; info.bytes as usize];
            f.read_exact(&mut b).map_err(|e| e.to_string())?;
            if sha256_hex(&b) != info.record_sha256 {
                return Err(format!(
                    "probe: record at {}+{} changed",
                    info.path, info.offset
                ));
            }
            pos.insert((ti, r), lines.len());
            lines.push(b);
        }
    }
    let table = rustred_campaign_core::economy::engine_probe_table(
        header.as_deref().unwrap_or(b""),
        &charts,
        &lines,
    );
    write(&dir.join("table.jsonl"), &table)?;
    // the sample: every key of a migrated rule, then a stride over the closure
    let rows = all_rows(set, res)?;
    let migrated: BTreeSet<&Vec<i32>> = decisions
        .iter()
        .filter(|d| d.outcome.migrates())
        .flat_map(|d| d.keys.iter().map(|k| &k.key))
        .collect();
    let plain = |r: &&rustred_campaign_core::import::KeyRow| {
        matches!(
            (&r.status, &r.server),
            (RowStatus::Served, Some(Server::Rule { .. }))
                | (RowStatus::Served, Some(Server::Relation { .. }))
        )
    };
    let mut sample: Vec<&rustred_campaign_core::import::KeyRow> =
        rows.iter().filter(|r| migrated.contains(&r.key)).collect();
    let others: Vec<&rustred_campaign_core::import::KeyRow> = rows
        .iter()
        .filter(plain)
        .filter(|r| !migrated.contains(&r.key))
        .collect();
    let want = cfg.engine_probe_sample.saturating_sub(sample.len());
    if want > 0 && !others.is_empty() {
        let stride = (others.len() / want).max(1);
        sample.extend(others.iter().step_by(stride).take(want));
    }
    let mut keys = String::new();
    let mut expected: Vec<Option<usize>> = vec![];
    for r in &sample {
        keys.push_str(&format!("{}\n", json!(r.key)));
        expected.push(match &r.server {
            Some(Server::Rule { table, record }) => pos.get(&(*table, *record)).copied(),
            _ => None,
        });
    }
    write(&dir.join("keys.jsonl"), keys.as_bytes())?;
    write(
        &dir.join("expected.json"),
        &serde_json::to_vec(&json!({"expected": expected, "migrated_keys": migrated.len()}))
            .map_err(|e| e.to_string())?,
    )?;
    let mut out = json!({"dir": dir.display().to_string(), "keys": sample.len(), "migrated_keys": migrated.len(),
        "table_sha256": sha256_hex(&table), "keys_sha256": sha256_hex(keys.as_bytes()),
        "command": format!("{} first-match --table {} --keys {} > ENGINE_OUT", cfg.program.path.display(),
                           dir.join("table.jsonl").display(), dir.join("keys.jsonl").display())});
    if let Some(ep) = &cfg.engine_first_match {
        let text = std::fs::read_to_string(ep).map_err(|e| format!("{}: {e}", ep.display()))?;
        let mut agree = 0usize;
        let mut differ: Vec<Value> = vec![];
        let mut n = 0usize;
        for (i, l) in text.lines().filter(|l| !l.trim().is_empty()).enumerate() {
            let v: Value =
                serde_json::from_str(l).map_err(|e| format!("engine line {}: {e}", i + 1))?;
            let Some(r) = sample.get(i) else {
                return Err(format!("engine line {}: more answers than keys", i + 1));
            };
            let k: Vec<i32> =
                serde_json::from_value(v["key"].clone()).map_err(|e| e.to_string())?;
            if k != r.key {
                return Err(format!(
                    "engine line {}: key {k:?}, expected {:?}",
                    i + 1,
                    r.key
                ));
            }
            let got = v["position"].as_u64().map(|x| x as usize);
            n += 1;
            if got == expected[i] {
                agree += 1;
            } else if differ.len() < 50 {
                differ.push(json!({"key": r.key, "engine": got, "import": expected[i],
                                   "migrated": migrated.contains(&r.key)}));
            }
        }
        out["engine"] = json!({"output": ep.display().to_string(), "answers": n, "agree": agree,
            "differ": n - agree, "differences": differ, "complete": n == sample.len()});
    }
    Ok(out)
}

fn failed_trial(job: &Job, k: &[i32], verdict: &str, why: &str, bound: &Value) -> KeyTrial {
    KeyTrial {
        key: k.to_vec(),
        step: job.step.clone(),
        recipe_id: job.recipe_id.clone(),
        recipe_sha256: job.recipe_sha256.clone(),
        recipe_digest: None,
        verdict: verdict.into(),
        certified: false,
        relation_digest: None,
        seconds: None,
        rhs_terms: None,
        entry_bytes: None,
        pole_order_d4: None,
        rhs_outside_certified: None,
        why: Some(why.into()),
        bound: bound.clone(),
        solved_line: None,
        results: None,
    }
}

/// The recipe sources of the shipped spec: one per recipe used by a migrating rule's recipe / stored keys, assigned
/// exactly those keys (assignment side files written next to the recipes).
fn shipped_recipes(
    cfg: &ShipRunConfig,
    decisions: &[RuleDecision],
    recipe_files: &BTreeMap<String, (PathBuf, String, u64)>,
) -> Result<(Vec<ShippedRecipe>, usize), String> {
    // ONE source per recipe id, named exactly as the solve job's source (the import binds every answer line to it);
    // form `stored` when any of its keys is stored (placed before the tables under `over_table`: a recipe-form key of the
    // same recipe goes with them, a documented deviation from the engine's RulesFirst records)
    let mut by: BTreeMap<String, (bool, Vec<Vec<i32>>)> = BTreeMap::new();
    for d in decisions.iter().filter(|d| d.outcome.migrates()) {
        for k in &d.keys {
            if !matches!(k.form.as_str(), "recipe" | "stored") {
                continue;
            }
            let Some(rid) = &k.recipe_id else { continue };
            let e = by.entry(rid.clone()).or_default();
            e.0 |= k.form == "stored";
            e.1.push(k.key.clone());
        }
    }
    let mut out = vec![];
    let mut n = 0usize;
    for (rid, (stored, keys)) in by {
        let (path, sha, _) = recipe_files
            .get(&rid)
            .ok_or_else(|| format!("recipe {rid}: no file"))?;
        let a = format!("{}\n", json!({"recipe": rid, "keys": keys}));
        let ap = cfg
            .out_dir
            .join("ship/assign")
            .join(format!("{}.jsonl", crate::closeloop::ids::safe(&rid)));
        write(&ap, a.as_bytes())?;
        n += keys.len();
        out.push(ShippedRecipe {
            id: rid.clone(),
            path: path.display().to_string(),
            sha256: sha.clone(),
            form: if stored { "stored" } else { "recipe" }.into(),
            assignment: Some(FileRef {
                path: ap.display().to_string(),
                sha256: Some(sha256_hex(a.as_bytes())),
                manifest: None,
                family: None,
            }),
        });
    }
    Ok((out, n))
}

/// Apply mode: the shipped artifact (`out_dir/ship/`): the shipped spec, the kept-aside list, the engine recipe
/// records (explicit certified keys + certified relation digests + the work bound), the stored relations as
/// `rustred.relation-entry.v1`, the removed list (revert), SHA256SUMS.
fn write_artifact(
    cfg: &ShipRunConfig,
    census: &Census,
    decisions: &[RuleDecision],
    spec: Option<&TableSetSpec>,
    records_by_key: &BTreeMap<Vec<i32>, Value>,
    set: &ImportedSet,
    extra: &Value,
) -> Result<Value, String> {
    let dir = cfg.out_dir.join("ship");
    let mut sums = String::new();
    let mut put = |name: &str, b: &[u8]| -> Result<(), String> {
        write(&dir.join(name), b)?;
        sums.push_str(&format!("{}  {name}\n", sha256_hex(b)));
        Ok(())
    };
    if let Some(s) = spec {
        put(
            "spec.json",
            &serde_json::to_vec_pretty(&ship::spec_json(s)).map_err(|e| e.to_string())?,
        )?;
    }
    let mut kept = vec![];
    let mut removed = vec![];
    let mut recipe_records: BTreeMap<String, Value> = BTreeMap::new();
    let mut entries = vec![];
    let fam_text = family_source_of(set)?;
    let node_identity = blake3::hash(
        json!({"family_blake3": blake3::hash(fam_text.as_bytes()).to_hex().to_string(),
               "order": serde_json::from_str::<Value>(&cfg.order).unwrap_or(Value::String(cfg.order.clone())),
               "arity": set.frame.arity()})
        .to_string()
        .as_bytes(),
    )
    .to_hex()
    .to_string();
    for (i, d) in decisions.iter().enumerate() {
        if !d.outcome.migrates() {
            continue;
        }
        let svc = &census.rules[i];
        let info = set
            .find_sha(&svc.rule.record_sha256)
            .map(|(t, r)| &set.rules[t][r]);
        let row = json!({"rule": d.rule, "outcome": d.outcome, "bytes": d.bytes_before,
            "table_path": info.map(|x| x.path.clone()), "offset": info.map(|x| x.offset)});
        if d.outcome == Outcome::KeptAside {
            kept.push(row.clone());
        }
        removed.push(row);
        for k in &d.keys {
            let Some(rec) = records_by_key.get(&k.key) else {
                continue;
            };
            match k.form.as_str() {
                "recipe" => {
                    let rid = k.recipe_id.clone().unwrap_or_default();
                    let e = recipe_records.entry(rid.clone()).or_insert_with(|| {
                        json!({"id": rid, "recipe_path": null, "recipe_digest": rec["recipe_digest"],
                               "domain": {"keys": []}, "relation_digests": {},
                               "max_trace_rows": cfg.policy.recipe.max_trace_rows})
                    });
                    if let Some(a) = e["domain"]["keys"].as_array_mut() {
                        a.push(json!(k.key));
                    }
                    e["relation_digests"][ship::key_text(&k.key)] = rec["relation_digest"].clone();
                }
                "stored" => {
                    let body = entry_body(rec);
                    let canon = rustred_campaign_core::schema::canon::canonical_json(&body, true)?;
                    entries.push(json!({"schema": "rustred.relation-entry.v1", "node": cfg.node,
                        "node_identity": node_identity, "key_frame": "table", "key": k.key,
                        "entry_digest": sha256_hex(canon.as_bytes()), "body": body,
                        "recipe": {"id": k.recipe_id, "file_sha256": null, "engine_digest": rec["recipe_digest"]},
                        "bound_to": {"determinism_id": rec["determinism_id"], "node_identity": node_identity,
                                     "relation_rule": rec["relation_rule"]},
                        "evidence": {"level": "per-key", "proof": {"tool": "rustred.recipe-solve/exact-weights",
                                     "record_sha256": sha256_hex(canon.as_bytes()), "verdict": "SOLVED"}},
                        "diagnostics": {"pole_order_at": {"4": rec["pole_order_d4"]}, "recipe": k.recipe_id},
                        "written": {"by": "rustred-campaign shipecon"}}));
                }
                _ => {}
            }
        }
    }
    put("kept_aside.jsonl", &jsonl(&kept))?;
    put("removed.jsonl", &jsonl(&removed))?;
    let rr: Vec<&Value> = recipe_records.values().collect();
    put("recipes.records.jsonl", &jsonl(&rr))?;
    put("relations.entries.jsonl", &jsonl(&entries))?;
    // plan/ARTIFACT_MANIFEST.md (rustred.node-service-manifest.v1, draft of vk5l-wp2): relations/<node>-<id16>/ with the
    // merged entry file, its index (key, file, offset, len, entry_digest) and MANIFEST.json
    let rel = format!(
        "relations/{}-{}",
        crate::closeloop::ids::safe(&cfg.node),
        node_identity.get(..16).unwrap_or(&node_identity)
    );
    let mut merged = Vec::new();
    let mut index = Vec::new();
    for e in &entries {
        let line = serde_json::to_string(e).map_err(|x| x.to_string())?;
        index.push(
            json!({"key": e["key"], "file": "merged-shipecon.jsonl", "offset": merged.len(),
            "len": line.len(), "entry_digest": e["entry_digest"]}),
        );
        merged.extend_from_slice(line.as_bytes());
        merged.push(b'\n');
    }
    put(&format!("{rel}/merged-shipecon.jsonl"), &merged)?;
    put(&format!("{rel}/index.jsonl"), &jsonl(&index))?;
    let pole_regular = entries
        .iter()
        .filter(|e| e["diagnostics"]["pole_order_at"]["4"] == 0)
        .count();
    let archive: Vec<Value> = removed
        .iter()
        .filter_map(|r| r["table_path"].as_str())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|p| {
            let rules = set.rules.iter().flatten().filter(|i| i.path == p).count();
            json!({"path": p, "sha256": set.rules.iter().flatten().find(|i| i.path == p)
                .and_then(|i| set.table_digests.get(&i.table_name)), "rules": rules})
        })
        .collect();
    let removed_labels: Vec<Value> = removed.iter().map(|r| r["rule"]["label"].clone()).collect();
    let removed_bytes: u64 = removed.iter().filter_map(|r| r["bytes"].as_u64()).sum();
    let total_rules: usize = set.rules.iter().map(Vec::len).sum();
    let manifest = json!({
        "schema": "rustred.node-service-manifest.v1",
        "node": cfg.node, "node_identity": node_identity, "key_frame": "table",
        "revision": format!("shipecon-{}", cfg.policy.decision_digest().get(..12).unwrap_or("")),
        "certified_set": {"keys": extra["certified_keys"], "source": extra["source"],
                          "encoding": "the owner payload's certified key set (unchanged by the ship economy)"},
        "table": {"shipped_rules": total_rules - removed.len(),
                  "archive": if archive.len() == 1 { archive[0].clone() } else { json!(archive) },
                  "removed": removed_labels, "removed_bytes": removed_bytes, "flagged": []},
        "entries": {"count": entries.len(), "bytes": merged.len(), "index": "index.jsonl",
                    "files": [{"file": "merged-shipecon.jsonl", "sha256": sha256_hex(&merged), "entries": entries.len()}],
                    "precedence": match cfg.policy.stored.precedence {
                        ship::Precedence::AfterTable => "after_table",
                        ship::Precedence::OverTable => "over_table",
                    },
                    "kinds": {"recipe_solve": entries.len(), "import": 0}},
        "recipes": {"records": rr.len(), "in_payload": false, "outside_families": true,
                    "file": "recipes.records.jsonl"},
        "services": extra["services"],
        "verification": {"entries_checked": entries.len(),
            "full_reduction_identical": null,
            "full_reduction_note": "not computed by the ship economy: each entry is the CERTIFIED relation of its key (exact \
                proof, regular, strictly lower, rhs inside the certified set) and the shipped closure was re-checked \
                (same start, same terminals, every originally served key served at the same or a higher level)",
            "rhs_inside_closure": if cfg.policy.coverage.require_rhs_in_certified_set {
                json!(entries.len())
            } else {
                Value::Null
            },
            "d4": {"entry_regular": pole_regular, "entry_pole": entries.len() - pole_regular, "rule_pole_at_key": null},
            "tool": format!("rustred-campaign shipecon (frozen engine {})", cfg.program.path.display())},
    });
    put(
        &format!("{rel}/MANIFEST.json"),
        &serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?,
    )?;
    std::fs::write(dir.join("SHA256SUMS"), sums.as_bytes()).map_err(|e| e.to_string())?;
    Ok(
        json!({"dir": dir.display().to_string(), "kept_aside": kept.len(), "removed": removed.len(),
        "recipe_records": rr.len(), "stored_entries": entries.len(), "node_identity": node_identity}),
    )
}

/// The frame table's `original_family_source` (else `family_source`) header text. procfix7 round 16 (int8b-gate review
/// of the tripwire-flagged lines, defect 2): a table that cannot be opened or read is an ERROR (the shipped spec's node
/// identity is never the hash of an empty family because of an I/O error).
fn family_source_of(set: &ImportedSet) -> Result<String, String> {
    let Some(i) = set.rules.iter().flatten().next() else {
        return Ok(String::new());
    };
    family_source_from(Path::new(&i.path))
}

fn family_source_from(path: &Path) -> Result<String, String> {
    let f = std::fs::File::open(path)
        .map_err(|e| format!("family source of {}: {e}", path.display()))?;
    let mut first = String::new();
    std::io::BufReader::new(f)
        .read_line(&mut first)
        .map_err(|e| format!("family source of {}: {e}", path.display()))?;
    let h: Value = serde_json::from_str(&first).unwrap_or(Value::Null);
    Ok(h["original_family_source"]
        .as_str()
        .or(h["family_source"].as_str())
        .unwrap_or("")
        .to_string())
}

#[cfg(test)]
mod family_source_tests {
    use super::*;

    /// procfix7 round 16 (int8b-gate review, defect 2): a frame table that cannot be opened (gone) or read (here: a
    /// directory, EISDIR at the read) is an error; a readable one gives its header's family source.
    #[test]
    fn an_unreadable_frame_table_is_an_error_never_an_empty_family() {
        let d = std::env::temp_dir().join(format!("rr-shipecon-family-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        assert!(family_source_from(&d.join("absent.jsonl")).is_err());
        std::fs::create_dir(d.join("dir.jsonl")).unwrap();
        assert!(family_source_from(&d.join("dir.jsonl")).is_err());
        std::fs::write(
            d.join("t.jsonl"),
            b"{\"family_source\":\"fam\",\"original_family_source\":\"orig\"}\n{}\n",
        )
        .unwrap();
        assert_eq!(family_source_from(&d.join("t.jsonl")).unwrap(), "orig");
        let _ = std::fs::remove_dir_all(&d);
    }
}
