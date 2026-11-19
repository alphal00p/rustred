//! The SHIP ECONOMY as SUPERVISED work (lane shipecon; interfaces/shipecon.md 13): a campaign whose input of role
//! `shipecon` names, per node, the material of a closed node (import spec, statement rows, family, order, the solve
//! program's key, the ship policy). Per node: the state is prepared in a pass thread (census, candidacy, plans), every
//! ROUND of the bounded guard becomes `ship-guard` ITEMS in the ledger (one per solve-recipe job: its batch registered
//! as an input), the supervisor runs the jobs through its single launch path (admission, containment, heartbeats,
//! stop / resume), each settled attempt goes through the closeloop accept chain and its trials are registered as an
//! answer input; when no trial is missing the decision and the closing check run in a pass thread and the report is
//! registered (`shipecon:<n>:report`). RESUME = the ledger: a restarted campaign re-prepares the state, applies every
//! registered answer and continues with the items not final and the rounds not created.
//! Generic (L8): nodes, paths, orders and bounds are data.

use super::ids::safe;
use super::shipecon::{Job, JobResult, ShipFake, ShipRunConfig, ShipState};
use super::termmin::cost_of;
use crate::adapters::job::{Invocation, Program};
use crate::ledger::{AcceptSource, Event, ItemEvent, LevelEvidence, NodeSpecValues};
use crate::state::{AttemptState, CampaignState, EvidenceLevel, ItemPhase, NodeEvent, Rung};
use crate::supervisor::spec::CampaignSpec;
use crate::supervisor::worksource::{
    AcceptedRecord, CampaignOutcome, JobRequest, Routing, Settled, SourceContext, WorkSource,
};
use rustred_campaign_contract::spec::{Budgets as JobBudgets, InputRef, JobKind};
use rustred_campaign_core::economy::ship::{KeyTrial, ShipPolicy};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::thread::JoinHandle;

/// Campaign input role of the ship campaign file.
pub const ROLE: &str = "shipecon";
pub const CAMPAIGN_SCHEMA: &str = "rustred.campaign.shipecon-campaign.v1";
pub const KIND_GUARD: &str = "ship-guard";
pub const ROLE_SPEC: &str = "shipecon_import_spec";
pub const ROLE_BATCH: &str = "shipecon_batch";
pub const ROLE_ANSWER: &str = "shipecon_answer";
pub const ROLE_REPORT: &str = "shipecon_report";
pub const ANSWER_SCHEMA: &str = "rustred.campaign.shipecon-answer.v1";

pub fn input_spec(n: &str) -> String {
    format!("shipecon:{n}:import_spec")
}
pub fn input_batch(item: &str) -> String {
    format!("shipecon:{item}:batch")
}
pub fn input_answer(item: &str, job: &str) -> String {
    format!("shipecon:{item}:answer:{job}")
}
pub fn input_report(n: &str) -> String {
    format!("shipecon:{n}:report")
}
pub fn guard_item(n: &str, round: usize, j: usize) -> String {
    format!("{n}.ship.r{round}.{j}")
}
/// The round of a guard item of node `n`.
pub fn parse_guard_item(n: &str, item: &str) -> Option<usize> {
    let rest = item.strip_prefix(n)?.strip_prefix(".ship.r")?;
    rest.split_once('.')?.0.parse().ok()
}

fn d_probe() -> usize {
    2000
}
fn d_threads() -> usize {
    4
}
fn d_tool() -> String {
    "tool".into()
}
fn d_full() -> String {
    "full".into()
}
fn d_cap() -> u64 {
    8 << 30
}
fn d_peak() -> u64 {
    1 << 30
}

/// Test stand: a key the fake child answers SOLVED under a step, with its right-hand-side keys.
pub type FakeAnswer = (Vec<i32>, Vec<Vec<i64>>);

/// One node of the ship campaign (data).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeEntry {
    pub node: String,
    pub import_spec: PathBuf,
    #[serde(default)]
    pub import_base: Option<PathBuf>,
    #[serde(default)]
    pub statement_rows: Option<PathBuf>,
    #[serde(default = "d_tool")]
    pub statement_format: String,
    #[serde(default)]
    pub rule_index: Option<PathBuf>,
    #[serde(default = "d_full")]
    pub check_mode: String,
    #[serde(default)]
    pub policy: Option<Value>,
    /// Key of `spec.programs` (the engine with the solve-recipe job mode).
    pub program: String,
    pub family: PathBuf,
    pub order: Value,
    #[serde(default)]
    pub previous: Option<PathBuf>,
    #[serde(default)]
    pub table_times: Option<PathBuf>,
    #[serde(default)]
    pub later_rule_probe: Option<PathBuf>,
    #[serde(default)]
    pub final_probe: Option<PathBuf>,
    #[serde(default = "d_probe")]
    pub engine_probe_sample: usize,
    #[serde(default = "d_threads")]
    pub check_threads: usize,
    #[serde(default = "d_cap")]
    pub cap_bytes: u64,
    #[serde(default = "d_peak")]
    pub expected_peak_bytes: u64,
    /// Test stand: per step a list of [key, [rhs keys]] the fake child answers SOLVED (others: no relation).
    #[serde(default)]
    pub fake: Option<BTreeMap<String, Vec<FakeAnswer>>>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CampaignFile {
    pub schema: String,
    pub nodes: Vec<NodeEntry>,
}

struct ShipNode {
    entry: NodeEntry,
    n: String,
    cfg: ShipRunConfig,
    state: Option<ShipState>,
    loading: Option<JoinHandle<Result<ShipState, String>>>,
    finishing: Option<JoinHandle<Result<Value, String>>>,
    /// Answer inputs already applied to the state.
    applied: BTreeSet<String>,
    /// Settled attempts that arrived before the state was loaded (applied in poll).
    pending: Vec<Settled>,
    failed: Option<String>,
}

/// The ship campaign source.
pub struct ShipCampaignSource {
    nodes: Vec<ShipNode>,
    cdir: PathBuf,
    cfg_input: String,
    programs: BTreeMap<String, PathBuf>,
}

fn json_bytes(v: &Value) -> Vec<u8> {
    let mut b = serde_json::to_vec_pretty(v).unwrap_or_default();
    b.push(b'\n');
    b
}

fn record(path: String, bytes: Vec<u8>) -> AcceptedRecord {
    AcceptedRecord { path, bytes }
}

fn blake3_hex(b: &[u8]) -> String {
    blake3::hash(b).to_hex().to_string()
}

impl ShipCampaignSource {
    /// The source of a campaign spec with an input of role [`ROLE`].
    pub fn from_spec(spec: &CampaignSpec, cdir: &Path) -> Result<ShipCampaignSource, String> {
        let pin: Vec<_> = spec.inputs.iter().filter(|i| i.role == ROLE).collect();
        let [p] = pin.as_slice() else {
            return Err(format!("shipecon: exactly one input of role {ROLE}"));
        };
        let text = std::fs::read_to_string(&p.path)
            .map_err(|e| format!("shipecon campaign {}: {e}", p.path.display()))?;
        let file: CampaignFile =
            serde_json::from_str(&text).map_err(|e| format!("shipecon campaign: {e}"))?;
        if file.schema != CAMPAIGN_SCHEMA {
            return Err(format!(
                "shipecon campaign: schema {:?}, expected {CAMPAIGN_SCHEMA}",
                file.schema
            ));
        }
        let programs: BTreeMap<String, PathBuf> = spec
            .programs
            .iter()
            .map(|(k, p)| (k.clone(), p.path.clone()))
            .collect();
        let mut nodes = vec![];
        for e in file.nodes {
            let n = safe(&e.node);
            let ppath = programs
                .get(&e.program)
                .ok_or_else(|| format!("node {}: program {} not in the spec", e.node, e.program))?;
            let program = Program::from_file(&e.program, ppath, Invocation::Contract)
                .map_err(|x| format!("node {}: program: {x:?}", e.node))?;
            let policy = match &e.policy {
                Some(v) => ShipPolicy::parse(&v.to_string())?,
                None => ShipPolicy::default(),
            };
            let dir = cdir.join("shipecon").join(&n);
            let base = e.import_base.clone().unwrap_or_else(|| {
                e.import_spec
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| PathBuf::from("/"))
            });
            let fake = e.fake.as_ref().map(|m| ShipFake {
                answers: m
                    .iter()
                    .map(|(s, v)| (s.clone(), v.iter().cloned().collect()))
                    .collect(),
                ms: 1,
            });
            let cfg = ShipRunConfig {
                node: e.node.clone(),
                import_spec: e.import_spec.clone(),
                import_base: base,
                statement_rows: e.statement_rows.clone(),
                policy,
                program,
                family: e.family.clone(),
                order: match &e.order {
                    Value::String(s) => s.clone(),
                    v => v.to_string(),
                },
                parallel: 1,
                threads_per_job: 1,
                cap_bytes: e.cap_bytes,
                wall_cap_s: 3600,
                work_root: dir.join("work"),
                out_dir: dir.join("out"),
                check_threads: e.check_threads,
                licence_env: None,
                previous: e.previous.clone(),
                table_times: e.table_times.clone(),
                engine_probe_sample: e.engine_probe_sample,
                engine_first_match: None,
                statement_format: e.statement_format.clone(),
                rule_index: e.rule_index.clone(),
                check_mode: e.check_mode.clone(),
                later_rule_probe: e.later_rule_probe.clone(),
                final_probe: e.final_probe.clone(),
                resume_trials: None,
                previous_solved: None,
                previous_work: None,
                fake,
                verbose: false,
            };
            nodes.push(ShipNode {
                entry: e,
                n,
                cfg,
                state: None,
                loading: None,
                finishing: None,
                applied: BTreeSet::new(),
                pending: vec![],
                failed: None,
            });
        }
        Ok(ShipCampaignSource {
            nodes,
            cdir: cdir.to_path_buf(),
            cfg_input: p.id.clone(),
            programs,
        })
    }

    fn abs(&self, rel: &str) -> PathBuf {
        self.cdir.join(rel)
    }

    fn node_of_item(&self, st: &CampaignState, item: &str) -> Option<usize> {
        let it = st.items.get(item)?;
        if it.kind != KIND_GUARD {
            return None;
        }
        self.nodes.iter().position(|p| p.entry.node == it.node)
    }

    fn read_input(&self, st: &CampaignState, id: &str) -> Result<Vec<u8>, String> {
        let r = st
            .inputs
            .get(id)
            .ok_or_else(|| format!("input {id} not registered"))?;
        let b = std::fs::read(&r.path).map_err(|e| format!("input {id} ({}): {e}", r.path))?;
        if blake3_hex(&b) != r.blake3 {
            return Err(format!(
                "input {id} ({}) changed since it was recorded",
                r.path
            ));
        }
        Ok(b)
    }

    fn batch(&self, st: &CampaignState, item: &str) -> Result<Job, String> {
        serde_json::from_slice(&self.read_input(st, &input_batch(item))?)
            .map_err(|e| format!("batch of {item}: {e}"))
    }

    fn request(
        &self,
        st: &CampaignState,
        i: usize,
        item: &str,
        job: String,
    ) -> Result<JobRequest, String> {
        let p = &self.nodes[i];
        let b = self.batch(st, item)?;
        let keys_rel = format!("shipecon/{}/batches/{item}.keys.jsonl", p.n);
        let keys_abs = self.abs(&keys_rel);
        let keys_b3 = crate::atomic::blake3_file(&keys_abs)
            .map_err(|e| e.to_string())?
            .1;
        let pol = &p.cfg.policy;
        let mut args = vec![
            "--order".to_string(),
            p.cfg.order.clone(),
            "--threads".into(),
            "1".into(),
        ];
        if let Some(x) = pol.recipe.max_trace_rows {
            args.extend(["--max-trace-rows".into(), x.to_string()]);
        }
        if let Some(x) = pol.recipe.key_budget_s {
            args.extend(["--key-budget-s".into(), x.to_string()]);
        }
        Ok(JobRequest {
            job,
            item: item.to_string(),
            rung: Rung(0),
            kind: JobKind::SolveRecipe,
            priority: -1,
            cap_bytes: p.entry.cap_bytes,
            expected_peak_bytes: p.entry.expected_peak_bytes,
            cores: 1,
            heartbeat_s: 10,
            stop_grace_s: 30,
            program: p.entry.program.clone(),
            args,
            inputs: vec![
                InputRef {
                    role: "family".into(),
                    path: p.cfg.family.display().to_string(),
                    blake3: crate::atomic::blake3_file(&p.cfg.family)
                        .map_err(|e| e.to_string())?
                        .1,
                    upto: None,
                },
                InputRef {
                    role: "recipe".into(),
                    path: b.recipe_path.display().to_string(),
                    blake3: crate::atomic::blake3_file(&b.recipe_path)
                        .map_err(|e| e.to_string())?
                        .1,
                    upto: None,
                },
                InputRef {
                    role: "keys".into(),
                    path: keys_abs.display().to_string(),
                    blake3: keys_b3,
                    upto: None,
                },
            ],
            depends_on: vec![input_batch(item)],
            budgets: JobBudgets::default(),
            sample_seed: 0,
            fake: p.cfg.fake.as_ref().map(|f| f.script(&b.step, &b.keys)),
        })
    }

    /// Apply every registered answer of node `i` not yet applied (resume, and answers routed before the state).
    fn apply_answers(&mut self, st: &CampaignState, i: usize) -> Result<(), String> {
        let node = self.nodes[i].entry.node.clone();
        let mut ids: Vec<String> = st
            .inputs
            .iter()
            .filter(|(id, r)| {
                r.role == ROLE_ANSWER
                    && id
                        .strip_prefix("shipecon:")
                        .and_then(|x| x.split(':').next())
                        .and_then(|item| st.items.get(item))
                        .is_some_and(|it| it.node == node)
            })
            .map(|(id, _)| id.clone())
            .collect();
        ids.sort();
        for id in ids {
            if self.nodes[i].applied.contains(&id) {
                continue;
            }
            let v: Value = serde_json::from_slice(&self.read_input(st, &id)?)
                .map_err(|e| format!("{id}: {e}"))?;
            let rule = v["rule"].as_u64().ok_or("answer without rule")? as usize;
            let ts: Vec<KeyTrial> =
                serde_json::from_value(v["trials"].clone()).map_err(|e| format!("{id}: {e}"))?;
            if let Some(state) = self.nodes[i].state.as_mut() {
                let t = state.trials.entry(rule).or_default();
                for nt in ts {
                    t.retain(|x| !(x.key == nt.key && x.step == nt.step));
                    t.push(nt);
                }
                self.nodes[i].applied.insert(id);
            }
        }
        Ok(())
    }

    /// Settle one guard attempt (state loaded): accept chain, trials, answer input, item events.
    fn settle(&mut self, st: &CampaignState, i: usize, done: &Settled) -> Result<Routing, String> {
        let mut rt = Routing::default();
        let job = self.batch(st, &done.item)?;
        let round = parse_guard_item(&self.nodes[i].n, &done.item).unwrap_or(0);
        let run_dir = st
            .jobs
            .get(&done.job)
            .and_then(|j| j.run_dir.clone())
            .unwrap_or_default();
        let c = &done.classification;
        let decided = matches!((c.row, c.sub.as_str()), (1, "") | (28, "") | (29, ""));
        let result: JobResult = match (&done.table, decided && !done.canonical_lines.is_empty()) {
            (Some(t), true) => Ok((
                self.abs(&t.path).display().to_string(),
                done.canonical_lines.clone(),
                t.blake3.clone(),
                run_dir.clone(),
                cost_of(Path::new(&run_dir), &done.job),
            )),
            _ => Err(format!("row {}{} ({:?}): {}", c.row, c.sub, c.kind, c.why)),
        };
        let cfg = self.nodes[i].cfg.clone();
        let state = self.nodes[i]
            .state
            .as_mut()
            .ok_or("shipecon: settled before the state was loaded")?;
        let job_rec = state.settle(&cfg, round, job.clone(), result)?;
        let trials: Vec<KeyTrial> = state
            .trials
            .get(&job.rule)
            .map(|v| {
                v.iter()
                    .filter(|t| t.step == job.step && job.keys.contains(&t.key))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        let certified: Vec<Vec<i32>> = trials
            .iter()
            .filter(|t| t.certified)
            .map(|t| t.key.clone())
            .collect();
        let n = self.nodes[i].n.clone();
        let a = record(
            format!("shipecon/{n}/answers/{}.{}.json", done.item, done.job),
            json_bytes(
                &json!({"schema": ANSWER_SCHEMA, "item": done.item, "job": done.job, "rule": job.rule,
                "step": job.step, "trials": trials, "job_rec": job_rec}),
            ),
        );
        let aref = a.file_ref();
        let aid = input_answer(&done.item, &done.job);
        rt.events.push(Event::InputRegistered {
            input: aid.clone(),
            role: ROLE_ANSWER.into(),
            path: self.abs(&a.path).display().to_string(),
            bytes: aref.bytes,
            blake3: aref.blake3.clone(),
        });
        self.nodes[i].applied.insert(aid);
        match (&done.table, certified.is_empty()) {
            (Some(table), false) => {
                let keys = record(
                    format!("shipecon/{n}/answers/{}.{}.keys.json", done.item, done.job),
                    json_bytes(&json!({"schema": "rustred.campaign.closeloop-keys.v1", "keys": certified})),
                );
                let kref = keys.file_ref();
                let canon = done
                    .canonical_blake3
                    .clone()
                    .unwrap_or_else(|| aref.blake3.clone());
                rt.events.push(Event::Item {
                    item: done.item.clone(),
                    ev: ItemEvent::Produce {
                        job: done.job.clone(),
                        attempt: done.attempt,
                        table: table.clone(),
                    },
                });
                rt.events.push(Event::Item {
                    item: done.item.clone(),
                    ev: ItemEvent::Accept {
                        rule: format!("{}:{}", done.item, &canon[..16.min(canon.len())]),
                        level: EvidenceLevel::IdentityProved,
                        record: aref.clone(),
                        source: AcceptSource::Job {
                            job: done.job.clone(),
                            attempt: done.attempt,
                            table_blake3: table.blake3.clone(),
                        },
                        elided: false,
                        evidence: Some(LevelEvidence::IdentityProved {
                            proofs: aref.clone(),
                            keys: kref,
                        }),
                        round: Some(round as u32),
                    },
                });
                rt.accepts.push(keys);
            }
            _ => rt.events.push(Event::Item {
                item: done.item.clone(),
                ev: ItemEvent::Escalate {
                    to: Rung(1),
                    reason: "shipecon: no key of the batch certified under this step (the next step is a new item)"
                        .into(),
                },
            }),
        }
        rt.accepts.push(a);
        Ok(rt)
    }

    /// Create the items of the next round of node `i` (or start the finishing thread when nothing is missing).
    fn next_round(&mut self, st: &CampaignState, i: usize, rt: &mut Routing) -> Result<(), String> {
        let node = self.nodes[i].entry.node.clone();
        let n = self.nodes[i].n.clone();
        let mut round = st
            .items
            .iter()
            .filter(|(_, it)| it.node == node && it.kind == KIND_GUARD)
            .filter_map(|(k, _)| parse_guard_item(&n, k))
            .max()
            .map_or(0, |r| r + 1);
        let cfg = self.nodes[i].cfg.clone();
        let steps = cfg.policy.recipe.steps.len();
        loop {
            let state = self.nodes[i].state.as_mut().ok_or("state not loaded")?;
            // the direct driver's bound (one round per step: each round tries the next step of the keys still open)
            let jobs = if round < steps {
                state.round_jobs(&cfg, round)?
            } else {
                None
            };
            match jobs {
                None => {
                    let state = self.nodes[i].state.take().ok_or("state not loaded")?;
                    let c2 = cfg.clone();
                    self.nodes[i].finishing = Some(
                        std::thread::Builder::new()
                            .name(format!("shipecon-finish-{n}"))
                            .spawn(move || state.finish(&c2))
                            .map_err(|e| format!("finish thread: {e}"))?,
                    );
                    return Ok(());
                }
                Some(v) if v.is_empty() => round += 1,
                Some(v) => {
                    for (j, job) in v.into_iter().enumerate() {
                        let item = guard_item(&n, round, j);
                        let keys_rel = format!("shipecon/{n}/batches/{item}.keys.jsonl");
                        let mut kb = String::new();
                        for k in &job.keys {
                            kb.push_str(&json!(k).to_string());
                            kb.push('\n');
                        }
                        let keys = record(keys_rel, kb.into_bytes());
                        let br = record(
                            format!("shipecon/{n}/batches/{item}.batch.json"),
                            json_bytes(&serde_json::to_value(&job).map_err(|e| e.to_string())?),
                        );
                        let f = br.file_ref();
                        // the files exist before the job request reads them (records are written before the line)
                        for r in [&keys, &br] {
                            let p = self.abs(&r.path);
                            if let Some(d) = p.parent() {
                                std::fs::create_dir_all(d)
                                    .map_err(|e| format!("{}: {e}", d.display()))?;
                            }
                            crate::atomic::write_atomic(&p, &r.bytes)
                                .map_err(|e| format!("{}: {e}", p.display()))?;
                        }
                        rt.events.push(Event::InputRegistered {
                            input: input_batch(&item),
                            role: ROLE_BATCH.into(),
                            path: self.abs(&br.path).display().to_string(),
                            bytes: f.bytes,
                            blake3: f.blake3,
                        });
                        rt.events.push(Event::ItemCreated {
                            item: item.clone(),
                            node: node.clone(),
                            kind: KIND_GUARD.into(),
                            rung: Rung(0),
                            parent: None,
                            inputs: vec![input_batch(&item)],
                            label: format!("ship guard {} {} keys", job.step, job.keys.len()),
                            weight: job.keys.len() as u64,
                        });
                    }
                    return Ok(());
                }
            }
        }
    }
}

impl WorkSource for ShipCampaignSource {
    fn bootstrap(
        &mut self,
        st: &CampaignState,
        _ctx: &SourceContext,
    ) -> Result<Vec<Event>, String> {
        let mut ev = vec![];
        for p in &self.nodes {
            let b = std::fs::read(&p.cfg.import_spec)
                .map_err(|e| format!("{}: {e}", p.cfg.import_spec.display()))?;
            let d = blake3_hex(&b);
            let sid = input_spec(&p.n);
            match st.inputs.get(&sid) {
                Some(x) if x.blake3 != d => {
                    return Err(format!(
                        "node {}: the import spec changed since the campaign started: refused",
                        p.entry.node
                    ));
                }
                Some(_) => {}
                None => ev.push(Event::InputRegistered {
                    input: sid.clone(),
                    role: ROLE_SPEC.into(),
                    path: p.cfg.import_spec.display().to_string(),
                    bytes: b.len() as u64,
                    blake3: d,
                }),
            }
            if !st.nodes.contains_key(&p.entry.node) {
                ev.push(Event::NodeRegistered {
                    node: p.entry.node.clone(),
                    inputs: vec![self.cfg_input.clone(), sid],
                    terminal_bound: None,
                    spec_box: None,
                    spec: NodeSpecValues {
                        full_attempt_cpu_hours: Some(0),
                        full_attempt_fixer_rounds: Some(0),
                        ..NodeSpecValues::default()
                    },
                    reference: None,
                });
                ev.push(Event::Node {
                    node: p.entry.node.clone(),
                    ev: NodeEvent::Start,
                });
            }
        }
        Ok(ev)
    }

    fn ready(&mut self, st: &CampaignState, _ctx: &SourceContext, limit: usize) -> Vec<JobRequest> {
        let mut out = vec![];
        for i in 0..self.nodes.len() {
            if self.nodes[i].state.is_none() {
                continue;
            }
            let node = self.nodes[i].entry.node.clone();
            let items: Vec<(String, ItemPhase, Vec<String>)> = st
                .items
                .iter()
                .filter(|(_, it)| it.node == node && it.kind == KIND_GUARD)
                .map(|(k, it)| (k.clone(), it.phase.clone(), it.jobs.clone()))
                .collect();
            for (item, ph, jobs) in items {
                let failed = jobs
                    .iter()
                    .filter(|x| {
                        matches!(
                            st.jobs.get(*x).map(|r| r.state),
                            Some(AttemptState::Failed(_) | AttemptState::BudgetStopped(_))
                        )
                    })
                    .count();
                let job = match ph {
                    ItemPhase::Open => Some(format!("{item}-t{}", jobs.len())),
                    ItemPhase::InProgress => {
                        match jobs.last().and_then(|l| st.jobs.get(l)).map(|r| r.state) {
                            Some(AttemptState::Queued) => jobs.last().cloned(),
                            Some(AttemptState::Failed(_) | AttemptState::BudgetStopped(_))
                                if failed < 3 =>
                            {
                                Some(format!("{item}-t{}", jobs.len()))
                            }
                            _ => None,
                        }
                    }
                    _ => None,
                };
                if let Some(job) = job {
                    match self.request(st, i, &item, job) {
                        Ok(r) => out.push(r),
                        Err(e) => eprintln!("shipecon: no job for {item}: {e}"),
                    }
                }
            }
        }
        out.sort_by(|a, b| a.job.cmp(&b.job));
        out.truncate(limit);
        out
    }

    fn route(
        &mut self,
        st: &CampaignState,
        _ctx: &SourceContext,
        done: &Settled,
    ) -> Result<Routing, String> {
        let Some(i) = self.node_of_item(st, &done.item) else {
            return Err(format!(
                "shipecon: settled attempt of a foreign item {}",
                done.item
            ));
        };
        if self.nodes[i].state.is_none() {
            // after a restart, before the state is loaded again: applied in poll
            self.nodes[i].pending.push(done.clone());
            return Ok(Routing::default());
        }
        self.settle(st, i, done)
    }

    fn control(
        &mut self,
        st: &CampaignState,
        verb: &str,
        target: &str,
    ) -> Result<Vec<Event>, String> {
        if self.node_of_item(st, target).is_some() {
            return match verb {
                "retry" | "skip" => Ok(vec![]),
                v => Err(format!("verb {v} not supported by the ship campaign")),
            };
        }
        Err(format!("shipecon: unknown target {target}"))
    }

    fn poll(&mut self, st: &CampaignState, _ctx: &SourceContext) -> Result<Routing, String> {
        let mut rt = Routing::default();
        for i in 0..self.nodes.len() {
            let n = self.nodes[i].n.clone();
            let node = self.nodes[i].entry.node.clone();
            if st.inputs.contains_key(&input_report(&n)) {
                continue;
            }
            // a finished pass thread
            if self.nodes[i]
                .loading
                .as_ref()
                .is_some_and(|h| h.is_finished())
            {
                let h = self.nodes[i].loading.take().expect("loading");
                match h
                    .join()
                    .unwrap_or_else(|_| Err("shipecon prepare thread panicked".into()))
                {
                    Ok(s) => self.nodes[i].state = Some(s),
                    Err(e) => self.nodes[i].failed = Some(e),
                }
            }
            if self.nodes[i]
                .finishing
                .as_ref()
                .is_some_and(|h| h.is_finished())
            {
                let h = self.nodes[i].finishing.take().expect("finishing");
                let rep = h
                    .join()
                    .unwrap_or_else(|_| Err("shipecon finish thread panicked".into()));
                let v = match rep {
                    Ok(v) => v,
                    Err(e) => json!({"error": e}),
                };
                let r = record(format!("shipecon/{n}/report.json"), json_bytes(&v));
                let f = r.file_ref();
                rt.events.push(Event::InputRegistered {
                    input: input_report(&n),
                    role: ROLE_REPORT.into(),
                    path: self.abs(&r.path).display().to_string(),
                    bytes: f.bytes,
                    blake3: f.blake3,
                });
                rt.events.push(Event::Note {
                    text: format!(
                        "shipecon {node}: report registered (closing check holds: {})",
                        v["closing_check"]["holds"]
                    ),
                });
                rt.accepts.push(r);
                continue;
            }
            if let Some(e) = self.nodes[i].failed.take() {
                let r = record(
                    format!("shipecon/{n}/report.json"),
                    json_bytes(&json!({"error": e})),
                );
                let f = r.file_ref();
                rt.events.push(Event::InputRegistered {
                    input: input_report(&n),
                    role: ROLE_REPORT.into(),
                    path: self.abs(&r.path).display().to_string(),
                    bytes: f.bytes,
                    blake3: f.blake3,
                });
                rt.events.push(Event::Note {
                    text: format!("shipecon {node}: stopped: {e}"),
                });
                rt.accepts.push(r);
                continue;
            }
            if self.nodes[i].loading.is_some() || self.nodes[i].finishing.is_some() {
                continue;
            }
            if self.nodes[i].state.is_none() {
                let cfg = self.nodes[i].cfg.clone();
                self.nodes[i].loading = Some(
                    std::thread::Builder::new()
                        .name(format!("shipecon-prepare-{n}"))
                        .spawn(move || ShipState::prepare(&cfg))
                        .map_err(|e| format!("prepare thread: {e}"))?,
                );
                continue;
            }
            self.apply_answers(st, i)?;
            for d in std::mem::take(&mut self.nodes[i].pending) {
                let r = self.settle(st, i, &d)?;
                rt.events.extend(r.events);
                rt.accepts.extend(r.accepts);
            }
            if !rt.events.is_empty() {
                continue;
            }
            let busy = st.items.values().any(|it| {
                it.node == node
                    && it.kind == KIND_GUARD
                    && matches!(
                        it.phase,
                        ItemPhase::Open | ItemPhase::InProgress | ItemPhase::Produced
                    )
            });
            if !busy {
                self.next_round(st, i, &mut rt)?;
            }
        }
        Ok(rt)
    }

    fn finished(&self, st: &CampaignState) -> bool {
        self.nodes
            .iter()
            .all(|p| st.inputs.contains_key(&input_report(&p.n)) && p.finishing.is_none())
    }

    fn outcome(&self, st: &CampaignState) -> CampaignOutcome {
        let mut bad = vec![];
        for p in &self.nodes {
            let ok = st
                .inputs
                .get(&input_report(&p.n))
                // procfix9-ok: an unreadable report counts the node as bad: the campaign outcome fails (int8b-gate review (i))
                .and_then(|r| std::fs::read(&r.path).ok())
                .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                .is_some_and(|v| v["error"].is_null());
            if !ok {
                bad.push(p.entry.node.clone());
            }
        }
        if bad.is_empty() {
            CampaignOutcome::Closed
        } else {
            CampaignOutcome::Error(format!("shipecon: no report for {}", bad.join(", ")))
        }
    }

    fn describe(&self) -> String {
        format!(
            "ship economy of {} node(s) (programs {:?})",
            self.nodes.len(),
            self.programs.keys().collect::<Vec<_>>()
        )
    }
}
