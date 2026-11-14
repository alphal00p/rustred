//! `CloseFromMaterialSource`: the `WorkSource` that closes nodes from EXISTING material (interfaces/closeloop.md).
//! A pure fold of the reduced ledger state and of the files the state names by digest: every decision is a ledger
//! record, a restarted supervisor gets the same answers from the same state (resume = replay; U-1: same decisions).

use super::accept::{AcceptCtx, BATCH_SCHEMA, BatchFile, decide};
use super::config::{CONFIG_ROLE, CloseLoopConfig, NodeCfg, Target};
use super::ids::*;
use super::pass::{Batch, PassIn, PassOut, ProofPlan, prefix_dispatcher, run_pass};
use super::proofs::{self, PBATCH_SCHEMA, ProofBatch, ProofBatchFile, ProofOracle};
use crate::ledger::{AcceptSource, Event, FileRef, ItemEvent, LevelEvidence, NodeSpecValues};
use crate::state::{
    AttemptEndCause, AttemptState, Blockers, CampaignState, ClosureTarget, EvidenceLevel,
    ItemPhase, NodeEvent, NodePhase, Rung, rule_element,
};
use crate::supervisor::spec::CampaignSpec;
use crate::supervisor::worksource::{
    AcceptedRecord, JobRequest, Routing, Settled, SourceContext, WorkSource,
};
use rustred_campaign_contract::classify::recipe::Verdict;
use rustred_campaign_contract::fake::{FakeOp, FakeScript, OnSigterm, RssPlan};
use rustred_campaign_contract::spec::{Budgets as JobBudgets, InputRef, JobKind};
use rustred_campaign_core::cover::Dispatcher;
use rustred_campaign_core::import::ImportedSet;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Instant;

pub const PROGRESS_SCHEMA: &str = "rustred.campaign.closeloop-progress.v1";

/// Fake-child answers per (recipe source, key): verdict and right-hand-side keys (tests only).
type Oracle = BTreeMap<(String, Vec<i32>), (Verdict, Vec<Vec<i64>>)>;
/// Issued (recipe source, key) pairs, keys issued, jobs issued, items in attention.
type Issued = (BTreeSet<(String, Vec<i32>)>, u64, u64, u64);

/// lane closeloop2: (record, key) pairs given to proof (span) items and to fact items, proof jobs issued, proof items
/// in attention.
#[derive(Default)]
struct ProofIssued {
    spans: BTreeSet<(String, Vec<i32>)>,
    facts: BTreeSet<(String, Vec<i32>)>,
    jobs: u64,
    attention: u64,
}

enum PassResult {
    Loaded(Arc<ImportedSet>),
    Checked(Box<PassOut>),
}

struct Running {
    round: u32,
    handle: JoinHandle<Result<PassResult, String>>,
}

struct NodeRt {
    cfg: NodeCfg,
    n: String,
    spec_text: String,
    spec_rel: String,
    family_blake3: String,
    set: Option<Arc<ImportedSet>>,
    running: Option<Running>,
    prefixes: BTreeMap<String, Option<Dispatcher>>,
    batches: BTreeMap<String, BatchFile>,
    pins: BTreeMap<String, String>,
    oracle: Oracle,
    last_check: Option<Value>,
    pass_started: Option<Instant>,
    // lane closeloop2
    pbatches: BTreeMap<String, ProofBatchFile>,
    proof_oracle: ProofOracle,
    /// BLAKE3 of the proof config's family / cert / symmetry group files.
    proof_inputs: BTreeMap<String, (PathBuf, String)>,
}

/// The close-from-material work source.
pub struct CloseFromMaterialSource {
    cfg_input: String,
    cdir: PathBuf,
    nodes: Vec<NodeRt>,
    programs: BTreeMap<String, PathBuf>,
    sha256: BTreeMap<String, String>,
    blake3: BTreeMap<String, String>,
    last_progress: Option<(Instant, String)>,
}

fn blake3_hex(b: &[u8]) -> String {
    blake3::hash(b).to_hex().to_string()
}

fn file_sha256(p: &Path) -> Result<String, String> {
    let b = std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?;
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(&b);
    Ok(h.finish_hex())
}

fn file_blake3(p: &Path) -> Result<String, String> {
    crate::atomic::blake3_file(p)
        .map(|(_, d)| d)
        .map_err(|e| format!("{}: {e}", p.display()))
}

fn resolve(base: &Path, p: &str) -> PathBuf {
    let pb = PathBuf::from(p);
    if pb.is_absolute() { pb } else { base.join(pb) }
}

fn record(path: String, bytes: Vec<u8>) -> AcceptedRecord {
    AcceptedRecord { path, bytes }
}

fn json_bytes(v: &Value) -> Vec<u8> {
    let mut b = serde_json::to_vec_pretty(v).expect("json");
    b.push(b'\n');
    b
}

impl CloseFromMaterialSource {
    /// The source of a campaign spec with an input of role `closeloop` (supervisor change 6.2).
    pub fn from_spec(spec: &CampaignSpec, cdir: &Path) -> Result<Self, String> {
        let ins: Vec<_> = spec
            .inputs
            .iter()
            .filter(|i| i.role == CONFIG_ROLE)
            .collect();
        let [inp] = ins.as_slice() else {
            return Err(format!(
                "closeloop: exactly one input of role {CONFIG_ROLE} expected, {} found",
                ins.len()
            ));
        };
        let cfg = CloseLoopConfig::read(&inp.path)?;
        let programs: BTreeMap<String, PathBuf> = spec
            .programs
            .iter()
            .map(|(k, p)| (k.clone(), p.path.clone()))
            .collect();
        Self::new(cfg, inp.id.clone(), cdir, programs)
    }

    pub fn new(
        cfg: CloseLoopConfig,
        cfg_input: String,
        cdir: &Path,
        programs: BTreeMap<String, PathBuf>,
    ) -> Result<Self, String> {
        cfg.validate()?;
        let mut s = CloseFromMaterialSource {
            cfg_input,
            cdir: cdir.to_path_buf(),
            nodes: vec![],
            programs,
            sha256: BTreeMap::new(),
            blake3: BTreeMap::new(),
            last_progress: None,
        };
        for nc in &cfg.nodes {
            let prog = s.solve_program(nc).to_string();
            let path = s
                .programs
                .get(&prog)
                .cloned()
                .ok_or_else(|| format!("node {}: program {prog} not in the spec", nc.node))?;
            let sha = file_sha256(&path)?;
            s.sha256.insert(prog.clone(), sha.clone());
            s.blake3.insert(prog.clone(), file_blake3(&path)?);
            let n = safe(&nc.node);
            let text = std::fs::read_to_string(&nc.import_spec)
                .map_err(|e| format!("import spec {}: {e}", nc.import_spec.display()))?;
            let mut v: Value = serde_json::from_str(&text)
                .map_err(|e| format!("import spec {}: {e}", nc.import_spec.display()))?;
            let o = v.as_object_mut().ok_or("import spec: not an object")?;
            if let Some(r) = &nc.default_recipe {
                let srcs = o
                    .get_mut("sources")
                    .and_then(Value::as_array_mut)
                    .ok_or("import spec: no sources")?;
                // R49: the default recipe is the LAST recipe source (after every other source: slot Last)
                srcs.push(
                    json!({"id": "default-recipe", "kind": "recipe", "class": "recipe",
                    "path": r.display().to_string()}),
                );
            }
            if let Some(r) = &nc.required_evidence {
                o.insert("required_evidence".into(), Value::from(r.clone()));
            }
            let ev = o
                .entry("evidence")
                .or_insert_with(|| json!({}))
                .as_object_mut()
                .ok_or("import spec: evidence is not an object")?;
            ev.insert("expected_recipe_binary".into(), Value::from(sha));
            let spec_text = serde_json::to_string_pretty(&v).expect("json") + "\n";
            let mut oracle = BTreeMap::new();
            if let Some(f) = &nc.solve.fake {
                let t = std::fs::read_to_string(&f.oracle)
                    .map_err(|e| format!("fake oracle {}: {e}", f.oracle.display()))?;
                for l in t.lines().filter(|l| !l.trim().is_empty()) {
                    let x: Value = serde_json::from_str(l).map_err(|e| format!("oracle: {e}"))?;
                    let key: Vec<i32> =
                        serde_json::from_value(x["key"].clone()).map_err(|e| e.to_string())?;
                    let verdict: Verdict =
                        serde_json::from_value(x["verdict"].clone()).map_err(|e| e.to_string())?;
                    let rhs: Vec<Vec<i64>> =
                        serde_json::from_value(x.get("rhs").cloned().unwrap_or(json!([])))
                            .map_err(|e| e.to_string())?;
                    let src = x["source"].as_str().unwrap_or("").to_string();
                    oracle.insert((src, key), (verdict, rhs));
                }
            }
            let family_blake3 = file_blake3(&nc.solve.family)?;
            let mut proof_inputs = BTreeMap::new();
            let mut proof_oracle = BTreeMap::new();
            if let Some(pc) = &nc.proof {
                let prog = pc.program_key().to_string();
                let path = s.programs.get(&prog).cloned().ok_or_else(|| {
                    format!("node {}: proof program {prog} not in the spec", nc.node)
                })?;
                s.sha256.insert(prog.clone(), file_sha256(&path)?);
                s.blake3.insert(prog, file_blake3(&path)?);
                for (role, p) in [
                    ("family", &pc.family),
                    ("cert", &pc.cert),
                    ("symmetry_group", &pc.symmetry_group),
                ] {
                    if let Some(p) = p {
                        proof_inputs.insert(role.to_string(), (p.clone(), file_blake3(p)?));
                    }
                }
                if let Some(f) = &pc.fake {
                    proof_oracle = proofs::read_oracle(&f.oracle)?;
                }
            }
            s.nodes.push(NodeRt {
                cfg: nc.clone(),
                spec_rel: format!("{}/import-spec.json", node_dir(&n)),
                n,
                spec_text,
                family_blake3,
                set: None,
                running: None,
                prefixes: BTreeMap::new(),
                batches: BTreeMap::new(),
                pins: BTreeMap::new(),
                oracle,
                last_check: None,
                pass_started: None,
                pbatches: BTreeMap::new(),
                proof_oracle,
                proof_inputs,
            });
        }
        Ok(s)
    }

    fn solve_program<'a>(&self, nc: &'a NodeCfg) -> &'a str {
        nc.solve
            .fake
            .as_ref()
            .map(|f| f.program.as_str())
            .unwrap_or(nc.solve.program.as_str())
    }

    fn abs(&self, rel: &str) -> PathBuf {
        self.cdir.join(rel)
    }

    fn node_of(&self, node: &str) -> Option<usize> {
        self.nodes.iter().position(|r| r.cfg.node == node)
    }

    fn items_of<'a>(
        st: &'a CampaignState,
        node: &'a str,
        kind: &'a str,
    ) -> impl Iterator<Item = (&'a String, &'a crate::state::ItemRec)> + 'a {
        st.items
            .iter()
            .filter(move |(_, it)| it.node == node && it.kind == kind)
    }

    fn item_final(p: &ItemPhase) -> bool {
        !matches!(
            p,
            ItemPhase::Open | ItemPhase::InProgress | ItemPhase::Produced
        )
    }

    /// Rounds whose check is recorded.
    fn rounds_done(st: &CampaignState, n: &str) -> u32 {
        let p = input_check_prefix(n);
        st.inputs.keys().filter(|k| k.starts_with(&p)).count() as u32
    }

    fn input_file(&self, st: &CampaignState, id: &str) -> Result<Vec<u8>, String> {
        let r = st
            .inputs
            .get(id)
            .ok_or_else(|| format!("input {id} not registered"))?;
        let b = std::fs::read(&r.path).map_err(|e| format!("input {id} ({}): {e}", r.path))?;
        if blake3_hex(&b) != r.blake3 {
            return Err(format!(
                "input {id} ({}) changed since it was recorded: refused",
                r.path
            ));
        }
        Ok(b)
    }

    fn batch(&mut self, st: &CampaignState, i: usize, item: &str) -> Result<BatchFile, String> {
        if let Some(b) = self.nodes[i].batches.get(item) {
            return Ok(b.clone());
        }
        let b: BatchFile = serde_json::from_slice(&self.input_file(st, &input_batch(item))?)
            .map_err(|e| format!("batch of {item}: {e}"))?;
        self.nodes[i].batches.insert(item.to_string(), b.clone());
        Ok(b)
    }

    /// Answers inputs of node `i` in ledger order.
    fn answers(&self, st: &CampaignState, i: usize) -> Vec<(String, PathBuf, String)> {
        self.inputs_of_role(st, i, ROLE_ANSWERS)
    }

    /// Inputs `closeloop:<item>:<what>:<job>` of role `role` of node `i`'s items, in ledger order.
    fn inputs_of_role(
        &self,
        st: &CampaignState,
        i: usize,
        role: &str,
    ) -> Vec<(String, PathBuf, String)> {
        let node = &self.nodes[i].cfg.node;
        let mut v: Vec<_> = st
            .inputs
            .iter()
            .filter(|(id, r)| {
                r.role == role
                    && id
                        .strip_prefix("closeloop:")
                        .and_then(|x| x.split_once(':'))
                        .and_then(|(item, _)| st.items.get(item))
                        .is_some_and(|it| it.node == *node)
            })
            .map(|(id, r)| {
                (
                    r.registered_seq,
                    id.clone(),
                    PathBuf::from(&r.path),
                    r.blake3.clone(),
                )
            })
            .collect();
        v.sort();
        v.into_iter().map(|(_, id, p, b)| (id, p, b)).collect()
    }

    /// (recipe source, key) pairs of every solve item of the node, keys issued, jobs issued, items in attention.
    fn issued(&mut self, st: &CampaignState, i: usize) -> Result<Issued, String> {
        let node = self.nodes[i].cfg.node.clone();
        let items: Vec<(String, ItemPhase)> = Self::items_of(st, &node, KIND_SOLVE)
            .map(|(k, it)| (k.clone(), it.phase.clone()))
            .collect();
        let mut pairs = BTreeSet::new();
        let (mut keys, mut att) = (0u64, 0u64);
        for (item, ph) in &items {
            let b = self.batch(st, i, item)?;
            keys += b.keys.len() as u64;
            for k in b.keys {
                pairs.insert((b.source.clone(), k));
            }
            if matches!(ph, ItemPhase::Attention(_)) {
                att += 1;
            }
        }
        Ok((pairs, keys, items.len() as u64, att))
    }

    fn pbatch(&mut self, st: &CampaignState, i: usize, item: &str) -> Result<ProofBatchFile, String> {
        if let Some(b) = self.nodes[i].pbatches.get(item) {
            return Ok(b.clone());
        }
        let b: ProofBatchFile = serde_json::from_slice(&self.input_file(st, &input_pbatch(item))?)
            .map_err(|e| format!("proof batch of {item}: {e}"))?;
        self.nodes[i].pbatches.insert(item.to_string(), b.clone());
        Ok(b)
    }

    /// (record, key) pairs of every proof / fact item of the node, proof jobs, proof items in attention.
    fn proof_issued(&mut self, st: &CampaignState, i: usize) -> Result<ProofIssued, String> {
        let node = self.nodes[i].cfg.node.clone();
        let items: Vec<(String, ItemPhase)> = st
            .items
            .iter()
            .filter(|(_, it)| it.node == node && (it.kind == KIND_PROOF || it.kind == KIND_FACTS))
            .map(|(k, it)| (k.clone(), it.phase.clone()))
            .collect();
        let mut out = ProofIssued::default();
        for (item, ph) in &items {
            let b = self.pbatch(st, i, item)?;
            if b.kind == "facts" {
                out.facts.extend(b.pairs());
            } else {
                out.spans.extend(b.pairs());
            }
            if matches!(ph, ItemPhase::Attention(_)) {
                out.attention += 1;
            }
        }
        out.jobs = items.len() as u64;
        Ok(out)
    }

    /// The relation point-record tables of node `i` (path, BLAKE3, recipe source), in ledger order.
    fn relation_tables(&mut self, st: &CampaignState, i: usize) -> Vec<(PathBuf, String, String)> {
        let mut v = vec![];
        for (id, p, b3) in self.inputs_of_role(st, i, ROLE_RELATIONS) {
            let item = id
                .strip_prefix("closeloop:")
                .and_then(|x| x.split_once(':'))
                .map(|(it, _)| it.to_string())
                .unwrap_or_default();
            if let Ok(b) = self.batch(st, i, &item) {
                v.push((p, b3, b.source));
            }
        }
        v
    }

    /// What node `i` needs now: None (nothing), Some(0) (load the set only), Some(k) (check pass k).
    fn needs_pass(&self, st: &CampaignState, i: usize) -> Option<u32> {
        let r = &self.nodes[i];
        if r.running.is_some() || st.inputs.contains_key(&input_final(&r.n)) {
            return None;
        }
        let node = &r.cfg.node;
        let nr = st.nodes.get(node)?;
        if nr.state.phase == NodePhase::Registered || nr.state.phase == NodePhase::Stale {
            return None;
        }
        let imp = st.items.get(&import_item(&r.n))?;
        if imp.phase == ItemPhase::Open {
            return Some(1);
        }
        let busy = st.items.values().any(|it| {
            it.node == *node
                && (it.kind == KIND_SOLVE || it.kind == KIND_PROOF || it.kind == KIND_FACTS)
                && !Self::item_final(&it.phase)
        });
        if busy {
            return if r.set.is_none() { Some(0) } else { None };
        }
        Some(Self::rounds_done(st, &r.n) + 1)
    }

    fn start_pass(&mut self, st: &CampaignState, i: usize, round: u32) -> Result<(), String> {
        let cached = self.nodes[i].set.clone();
        if round == 0 {
            let r = &self.nodes[i];
            let text = r.spec_text.clone();
            let base = r.cfg.base();
            let h = std::thread::Builder::new()
                .name(format!("closeloop-load-{}", r.n))
                .spawn(move || {
                    let spec = rustred_campaign_core::import::TableSetSpec::from_json(&text)
                        .map_err(|e| format!("import spec: {e}"))?;
                    let set = rustred_campaign_core::import::load(&spec, &base)
                        .map_err(|e| format!("import load: {e}"))?;
                    Ok(PassResult::Loaded(Arc::new(set)))
                })
                .map_err(|e| format!("pass thread: {e}"))?;
            self.nodes[i].running = Some(Running { round, handle: h });
            self.nodes[i].pass_started = Some(Instant::now());
            return Ok(());
        }
        let (issued, keys_issued, jobs_issued, att) = self.issued(st, i)?;
        let answers = self
            .answers(st, i)
            .into_iter()
            .map(|(_, p, b)| (p, b))
            .collect();
        let pi = self.proof_issued(st, i)?;
        let proof_files = self
            .inputs_of_role(st, i, ROLE_PROOFS)
            .into_iter()
            .map(|(_, p, b)| (p, b))
            .collect();
        let fact_files = self
            .inputs_of_role(st, i, ROLE_FACTS)
            .into_iter()
            .map(|(_, p, b)| (p, b))
            .collect();
        let relation_tables = self.relation_tables(st, i);
        let r = &self.nodes[i];
        let inp = PassIn {
            node: r.cfg.node.clone(),
            round,
            spec_text: r.spec_text.clone(),
            base: r.cfg.base(),
            answers,
            threads: r.cfg.check_threads,
            required: None,
            issued,
            keys_issued,
            jobs_issued,
            budgets: r.cfg.budgets,
            max_keys_per_job: r.cfg.solve.max_keys_per_job,
            hand: r.cfg.hand.clone(),
            final_dir: self.abs(&format!("{}/final", node_dir(&r.n))),
            attention_items: att,
            relation_rule: r.cfg.solve.relation_rule.clone(),
            proof_files,
            fact_files,
            relation_tables,
            issued_proofs: pi.spans,
            issued_facts: pi.facts,
            proof_jobs_issued: pi.jobs,
            proof_attention: pi.attention,
            proof: r.cfg.proof.as_ref().map(|p| ProofPlan {
                import_requests: p.import_requests,
                relations: p.relations,
                facts: p.facts,
                max_keys_per_job: p.max_keys_per_job,
                max_fact_keys_per_job: p.max_fact_keys_per_job,
            }),
            exact_tier: r.cfg.exact_tier,
            rows_path: self.abs(&format!("{}/checks/r{round}.rows.jsonl", node_dir(&r.n))),
            target: match r.cfg.target {
                Target::Boxed => "boxed".into(),
                Target::Full => "full".into(),
            },
        };
        let h = std::thread::Builder::new()
            .name(format!("closeloop-pass-{}", r.n))
            .spawn(move || run_pass(&inp, cached).map(|o| PassResult::Checked(Box::new(o))))
            .map_err(|e| format!("pass thread: {e}"))?;
        self.nodes[i].running = Some(Running { round, handle: h });
        self.nodes[i].pass_started = Some(Instant::now());
        Ok(())
    }

    fn fake_script(&self, i: usize, b: &BatchFile) -> Option<FakeScript> {
        let r = &self.nodes[i];
        let f = r.cfg.solve.fake.as_ref()?;
        let mut ops: Vec<FakeOp> = f
            .quirks
            .iter()
            .map(|q| FakeOp::RecipeQuirk { quirk: *q })
            .collect();
        ops.push(FakeOp::Prepare { ms: 10 });
        for k in &b.keys {
            let (verdict, rhs) = r
                .oracle
                .get(&(b.source.clone(), k.clone()))
                .cloned()
                .unwrap_or((Verdict::NoRelationInSeeds, vec![]));
            ops.push(FakeOp::Relation {
                ms: f.ms,
                verdict,
                rhs,
            });
        }
        ops.push(FakeOp::End {
            stop: None,
            verdict: None,
        });
        Some(FakeScript {
            script: ops,
            on_sigterm: OnSigterm::Graceful,
            slow_ms: 0,
            start_delay_ms: 0,
            rss: RssPlan::default(),
            helpers: vec![],
            faults: vec![],
            time_scale: 1.0,
            volatile_jitter: false,
        })
    }

    fn request(
        &mut self,
        st: &CampaignState,
        i: usize,
        item: &str,
        job: String,
        priority: i64,
    ) -> Result<JobRequest, String> {
        let b = self.batch(st, i, item)?;
        let keys_rel = format!("{}/batches/{}.keys.jsonl", node_dir(&self.nodes[i].n), item);
        let keys_abs = self.abs(&keys_rel);
        let keys_b3 = file_blake3(&keys_abs)?;
        let r = &self.nodes[i];
        let sc = &r.cfg.solve;
        let mut args = vec![
            "--order".to_string(),
            sc.order_arg(),
            "--threads".into(),
            sc.threads.to_string(),
        ];
        if let Some(x) = sc.max_trace_rows {
            args.extend(["--max-trace-rows".into(), x.to_string()]);
        }
        if let Some(x) = sc.key_budget_s {
            args.extend(["--key-budget-s".into(), x.to_string()]);
        }
        Ok(JobRequest {
            job,
            item: item.to_string(),
            rung: Rung(0),
            kind: JobKind::SolveRecipe,
            priority,
            cap_bytes: sc.cap_bytes,
            expected_peak_bytes: sc.expected_peak_bytes,
            cores: sc.threads as usize,
            heartbeat_s: sc.heartbeat_s,
            stop_grace_s: sc.stop_grace_s,
            program: self.solve_program(&r.cfg).to_string(),
            args,
            inputs: vec![
                InputRef {
                    role: "family".into(),
                    path: sc.family.display().to_string(),
                    blake3: r.family_blake3.clone(),
                    upto: None,
                },
                InputRef {
                    role: "recipe".into(),
                    path: b.recipe_path.clone(),
                    blake3: b.recipe_blake3.clone(),
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
            fake: self.fake_script(i, &b),
        })
    }

    fn ensure_set(&mut self, i: usize) -> Result<Arc<ImportedSet>, String> {
        if let Some(s) = &self.nodes[i].set {
            return Ok(s.clone());
        }
        // only at recovery (a settled attempt routed before any pass ran): synchronous load
        let r = &self.nodes[i];
        let spec = rustred_campaign_core::import::TableSetSpec::from_json(&r.spec_text)
            .map_err(|e| format!("import spec: {e}"))?;
        let set = Arc::new(
            rustred_campaign_core::import::load(&spec, &r.cfg.base())
                .map_err(|e| format!("import load: {e}"))?,
        );
        self.nodes[i].set = Some(set.clone());
        Ok(set)
    }

    /// The engine digest pinned for a recipe source: the batch's (import spec), else the first answer recorded.
    fn pinned(&mut self, st: &CampaignState, i: usize, b: &BatchFile) -> Option<String> {
        if b.engine_digest.is_some() {
            return b.engine_digest.clone();
        }
        if let Some(p) = self.nodes[i].pins.get(&b.source) {
            return Some(p.clone());
        }
        for (id, _, _) in self.answers(st, i) {
            let Ok(bytes) = self.input_file(st, &id) else {
                continue;
            };
            for l in String::from_utf8_lossy(&bytes).lines() {
                let Ok(v) = serde_json::from_str::<Value>(l) else {
                    continue;
                };
                if v["source"] == b.source.as_str()
                    && let Some(d) = v["recipe_digest"].as_str()
                {
                    self.nodes[i].pins.insert(b.source.clone(), d.to_string());
                    return Some(d.to_string());
                }
            }
        }
        None
    }

    /// The events and records of a finished check pass of node `i`.
    fn commit_pass(
        &mut self,
        st: &CampaignState,
        i: usize,
        out: PassOut,
    ) -> Result<Routing, String> {
        let mut rt = Routing::default();
        let r = &self.nodes[i];
        let (n, node) = (r.n.clone(), r.cfg.node.clone());
        let boxed = r.cfg.target == Target::Boxed;
        let nr = st.nodes.get(&node).ok_or("node not registered")?;
        let k = out.round;
        // the check record
        let check_rel = format!("{}/checks/r{k}.json", node_dir(&n));
        let check_rec = record(check_rel.clone(), json_bytes(&out.check));
        let check_ref = check_rec.file_ref();
        rt.events.push(Event::InputRegistered {
            input: input_check(&n, k),
            role: ROLE_CHECK.into(),
            path: self.abs(&check_rel).display().to_string(),
            bytes: check_ref.bytes,
            blake3: check_ref.blake3.clone(),
        });
        rt.accepts.push(check_rec);
        // round 1: provenance, the import item, the dispatch order
        let mut rule_set = nr.rule_set.clone();
        let mut dispatch = nr.state.dispatch_order.clone();
        let imp = import_item(&n);
        if st
            .items
            .get(&imp)
            .is_some_and(|it| it.phase == ItemPhase::Open)
        {
            let man = record(
                format!("{}/import-manifest.json", node_dir(&n)),
                json_bytes(&out.import_manifest),
            );
            let dis = record(
                format!("{}/dispatch-order.json", node_dir(&n)),
                json_bytes(&out.dispatch_order),
            );
            let (mref, dref) = (man.file_ref(), dis.file_ref());
            let spec_ref = FileRef {
                path: self.nodes[i].spec_rel.clone(),
                bytes: self.nodes[i].spec_text.len() as u64,
                blake3: blake3_hex(self.nodes[i].spec_text.as_bytes()),
            };
            let prov = format!("closeloop:{n}:material");
            let digest = out.spec_digest.trim_start_matches("sha256:").to_string();
            let rule = format!("{imp}:{}", &digest[..16.min(digest.len())]);
            rt.events.push(Event::TablesImported {
                provenance: prov.clone(),
                producer: "rustred-campaign-core::import (lane closeloop)".into(),
                source_campaign: None,
                manifest: mref.clone(),
                files: vec![spec_ref],
                level_claimed: EvidenceLevel::GeometryAccepted,
            });
            rt.events.push(Event::Item {
                item: imp.clone(),
                ev: ItemEvent::Import {
                    rule: rule.clone(),
                    level: EvidenceLevel::GeometryAccepted,
                    record: mref.clone(),
                    provenance: prov,
                },
            });
            rule_set.add(rule_element(&imp, &rule, &mref.blake3, &[]));
            rt.events.push(Event::Node {
                node: node.clone(),
                ev: NodeEvent::Assembled {
                    round: 1,
                    manifest: mref,
                    dispatch_order: dref.clone(),
                },
            });
            dispatch = Some(dref.blake3.clone());
            rt.accepts.push(man);
            rt.accepts.push(dis);
        }
        let rsd = rule_set.digest();
        let dod = dispatch.ok_or("no dispatch order recorded for the node")?;
        let mut round = nr.state.closure_rounds;
        let mut result = |defect: u64| {
            round += 1;
            Event::Node {
                node: node.clone(),
                ev: NodeEvent::ClosureResult {
                    round,
                    defect_keys: defect,
                    closure_keys: out.closure_keys,
                    rules_used: Some(out.rules_used),
                    rules_total: Some(out.rules_total),
                    keyset: None,
                    rule_set_digest: rsd.clone(),
                    dispatch_order_digest: dod.clone(),
                    envelope: crate::ledger::EnvelopeKind::Measured,
                    envelope_tables: vec![],
                },
            }
        };
        let blockers = |list: &FileRef| Blockers {
            faces: 0,
            loci: 0,
            keys: out.defect,
            list: Some(list.clone()),
        };
        rt.events.push(result(out.defect));
        let mut on_full_closed = false;
        if boxed && nr.state.target == ClosureTarget::Full {
            if out.defect > 0 {
                // closeloop.md 13.3: the node is BOXED from its first record; the ledger has no boxed start (NodeState
                // is FULL at registration, FallBackToBox needs an ended attempt): this pair is the ledger's only legal
                // way into the spec box until the ledger owner adds `NodeSpecValues.start_target` (lane notes)
                rt.events.push(Event::Node {
                    node: node.clone(),
                    ev: NodeEvent::AttemptEnded {
                        blockers: blockers(&check_ref),
                        reason: "spec box from the start (closeloop.md 13.3): ledger transition only, no FULL attempt was made (the ledger has no boxed start)".into(),
                        cause: AttemptEndCause::FixerRounds,
                    },
                });
                rt.events.push(Event::Node {
                    node: node.clone(),
                    ev: NodeEvent::FallBackToBox {
                        box_id: input_box(&n),
                    },
                });
                rt.events.push(result(out.defect));
            } else {
                on_full_closed = true;
            }
        }
        // new solve items
        for (j, b) in out.batches.iter().enumerate() {
            self.new_item(&mut rt, i, k, j, b)?;
        }
        // lane closeloop2: new proof (span) and fact items
        for (j, b) in out.proof_batches.iter().enumerate() {
            self.new_proof_item(&mut rt, i, k, j, b)?;
        }
        // phase D
        if let Some(f) = &out.final_ {
            let fr = record(
                format!("{}/final/result.json", node_dir(&n)),
                json_bytes(&f.result),
            );
            let fref = fr.file_ref();
            rt.events.push(Event::InputRegistered {
                input: input_final(&n),
                role: ROLE_RESULT.into(),
                path: self.abs(&fr.path).display().to_string(),
                bytes: fref.bytes,
                blake3: fref.blake3.clone(),
            });
            rt.accepts.push(fr);
            if f.verdict == "closed" {
                if on_full_closed {
                    rt.events.push(Event::Note {
                        text: format!(
                            "closeloop {node}: closed at the first check; the node machine keeps it on the FULL target (it cannot box a node whose FULL attempt did not fail, R33): the box verdict is in {}",
                            fref.path
                        ),
                    });
                } else {
                    rt.events.push(Event::Node {
                        node: node.clone(),
                        ev: NodeEvent::DeclareClosed,
                    });
                }
            } else {
                rt.events.push(Event::Node {
                    node: node.clone(),
                    ev: NodeEvent::AttemptEnded {
                        blockers: blockers(&fref),
                        reason: format!("closeloop verdict {}", f.verdict),
                        cause: AttemptEndCause::FixerRounds,
                    },
                });
                let why: Vec<String> = f.result["blockers"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect();
                rt.events.push(Event::Node {
                    node: node.clone(),
                    ev: NodeEvent::DeclareOpen {
                        reason: format!("{}: {}", f.verdict, why.join("; ")),
                    },
                });
            }
        }
        self.nodes[i].last_check = Some(out.check.clone());
        self.nodes[i].set = Some(out.set);
        Ok(rt)
    }

    fn new_item(
        &mut self,
        rt: &mut Routing,
        i: usize,
        k: u32,
        j: usize,
        b: &Batch,
    ) -> Result<(), String> {
        let (n, node) = (self.nodes[i].n.clone(), self.nodes[i].cfg.node.clone());
        let item = solve_item(&n, k, j);
        let base = self.nodes[i].cfg.base();
        let rp = resolve(&base, &b.recipe_path);
        let bf = BatchFile {
            schema: BATCH_SCHEMA.into(),
            node: node.clone(),
            item: item.clone(),
            round: k,
            source: b.source.clone(),
            recipe_path: rp.display().to_string(),
            recipe_sha256: b.recipe_sha256.clone(),
            recipe_blake3: file_blake3(&rp)?,
            engine_digest: b.engine_digest.clone(),
            relation_rule: self.nodes[i].cfg.solve.relation_rule.clone(),
            keys: b.keys.clone(),
        };
        let mut kb = Vec::new();
        for key in &b.keys {
            kb.extend_from_slice(json!({ "key": key }).to_string().as_bytes());
            kb.push(b'\n');
        }
        let dir = node_dir(&n);
        let keys = record(format!("{dir}/batches/{item}.keys.jsonl"), kb);
        let bfr = record(
            format!("{dir}/batches/{item}.json"),
            json_bytes(&serde_json::to_value(&bf).expect("json")),
        );
        let r = bfr.file_ref();
        rt.events.push(Event::InputRegistered {
            input: input_batch(&item),
            role: ROLE_BATCH.into(),
            path: self.abs(&bfr.path).display().to_string(),
            bytes: r.bytes,
            blake3: r.blake3,
        });
        rt.events.push(Event::ItemCreated {
            item: item.clone(),
            node,
            kind: KIND_SOLVE.into(),
            rung: Rung(0),
            parent: Some(import_item(&n)),
            inputs: vec![input_batch(&item)],
            label: b.source.clone(),
            weight: b.keys.len() as u64,
        });
        rt.accepts.push(keys);
        rt.accepts.push(bfr);
        self.nodes[i].batches.insert(item, bf);
        Ok(())
    }

    /// lane closeloop2: one proof / fact item (its batch file and the job's input files are accepted records).
    fn new_proof_item(
        &mut self,
        rt: &mut Routing,
        i: usize,
        k: u32,
        j: usize,
        b: &ProofBatch,
    ) -> Result<(), String> {
        let (n, node) = (self.nodes[i].n.clone(), self.nodes[i].cfg.node.clone());
        let pc = self.nodes[i]
            .cfg
            .proof
            .clone()
            .ok_or("proof batch without a proof config")?;
        let item = proof_item(&n, k, j, &b.kind);
        let tp = PathBuf::from(&b.table);
        let bf = ProofBatchFile {
            schema: PBATCH_SCHEMA.into(),
            node: node.clone(),
            item: item.clone(),
            round: k,
            kind: b.kind.clone(),
            reason: b.reason.clone(),
            table: tp.display().to_string(),
            table_blake3: file_blake3(&tp)?,
            records: b.records.clone(),
            relation_source: b.relation_source.clone(),
            rows: pc.rows.clone(),
            drop_zero_sectors: b.relation_source.is_some() && pc.drop_zero_sectors_relations,
        };
        let dir = format!("{}/proofs", node_dir(&n));
        if bf.kind == "span" {
            rt.accepts.push(record(
                format!("{dir}/{item}.request.json"),
                proofs::span_request_bytes(&bf),
            ));
            rt.accepts.push(record(
                format!("{dir}/{item}.keys.txt"),
                proofs::span_keys_bytes(&bf),
            ));
        } else {
            rt.accepts.push(record(
                format!("{dir}/{item}.requests.jsonl"),
                proofs::fact_requests_bytes(&bf),
            ));
        }
        let bfr = record(
            format!("{dir}/{item}.json"),
            json_bytes(&serde_json::to_value(&bf).expect("json")),
        );
        let r = bfr.file_ref();
        rt.events.push(Event::InputRegistered {
            input: input_pbatch(&item),
            role: ROLE_PBATCH.into(),
            path: self.abs(&bfr.path).display().to_string(),
            bytes: r.bytes,
            blake3: r.blake3,
        });
        rt.events.push(Event::ItemCreated {
            item: item.clone(),
            node,
            kind: if bf.kind == "facts" {
                KIND_FACTS.into()
            } else {
                KIND_PROOF.into()
            },
            rung: Rung(0),
            parent: Some(import_item(&n)),
            inputs: vec![input_pbatch(&item)],
            label: format!("{}:{}", bf.kind, bf.reason),
            weight: b.keys(),
        });
        rt.accepts.push(bfr);
        self.nodes[i].pbatches.insert(item, bf);
        Ok(())
    }

    /// lane closeloop2: the job of a proof / fact item (legacy rr_proof; argv derived from the inputs by the supervisor).
    fn request_proof(
        &mut self,
        st: &CampaignState,
        i: usize,
        item: &str,
        job: String,
        priority: i64,
    ) -> Result<JobRequest, String> {
        let b = self.pbatch(st, i, item)?;
        let r = &self.nodes[i];
        let pc = r.cfg.proof.as_ref().ok_or("proof item without a proof config")?;
        let dir = format!("{}/proofs", node_dir(&r.n));
        let file = |rel: String, role: &str| -> Result<InputRef, String> {
            let p = self.abs(&rel);
            Ok(InputRef {
                role: role.into(),
                path: p.display().to_string(),
                blake3: file_blake3(&p)?,
                upto: None,
            })
        };
        let table = InputRef {
            role: "table".into(),
            path: b.table.clone(),
            blake3: b.table_blake3.clone(),
            upto: None,
        };
        let (kind, inputs) = if b.kind == "span" {
            let mut v = vec![
                file(format!("{dir}/{item}.request.json"), "request")?,
                table,
                file(format!("{dir}/{item}.keys.txt"), "keys")?,
            ];
            let rows_role = if pc.rows == "cert" { "cert" } else { "family" };
            for role in [rows_role, "symmetry_group"] {
                if role == "symmetry_group" && b.relation_source.is_none() {
                    continue; // imported records carry their own group records (rr_proof reads the table's)
                }
                if let Some((p, d)) = r.proof_inputs.get(role) {
                    v.push(InputRef {
                        role: role.into(),
                        path: p.display().to_string(),
                        blake3: d.clone(),
                        upto: None,
                    });
                }
            }
            (JobKind::ProofInstanceSpan, v)
        } else {
            (
                JobKind::ProofInstantiate,
                vec![file(format!("{dir}/{item}.requests.jsonl"), "requests")?, table],
            )
        };
        let fake = pc
            .fake
            .as_ref()
            .map(|f| proofs::fake_script(&b, &r.proof_oracle, f.ms));
        Ok(JobRequest {
            job,
            item: item.to_string(),
            rung: Rung(0),
            kind,
            priority,
            cap_bytes: pc.cap_bytes,
            expected_peak_bytes: pc.expected_peak_bytes,
            cores: pc.threads as usize,
            heartbeat_s: pc.heartbeat_s(),
            stop_grace_s: 5,
            program: pc.program_key().to_string(),
            args: vec![],
            inputs,
            depends_on: vec![input_pbatch(item)],
            budgets: JobBudgets::default(),
            sample_seed: 0,
            fake,
        })
    }

    fn error_final(&mut self, st: &CampaignState, i: usize, round: u32, e: &str) -> Routing {
        let n = self.nodes[i].n.clone();
        let node = self.nodes[i].cfg.node.clone();
        let mut rt = Routing::default();
        let v = json!({"schema": super::pass::RESULT_SCHEMA, "node": node, "verdict": "error", "rounds": round,
            "blockers": [format!("pass_failed: {e}")], "error": e});
        let fr = record(
            format!("{}/final/result.json", node_dir(&n)),
            json_bytes(&v),
        );
        let r = fr.file_ref();
        rt.events.push(Event::Note {
            text: format!("closeloop {node}: pass {round} failed: {e}"),
        });
        rt.events.push(Event::InputRegistered {
            input: input_final(&n),
            role: ROLE_RESULT.into(),
            path: self.abs(&fr.path).display().to_string(),
            bytes: r.bytes,
            blake3: r.blake3,
        });
        let imp = import_item(&n);
        if st
            .items
            .get(&imp)
            .is_some_and(|it| it.phase == ItemPhase::Open)
        {
            rt.events.push(Event::Item {
                item: imp,
                ev: ItemEvent::Attention {
                    code: "import_failed".into(),
                    text: e.to_string(),
                },
            });
        }
        rt.accepts.push(fr);
        rt
    }

    /// The derived progress file (closeloop.md 10).
    fn progress(&mut self, st: &CampaignState, finalized: &BTreeMap<usize, String>) {
        let mut nodes = vec![];
        for (i, r) in self.nodes.iter().enumerate() {
            let node = &r.cfg.node;
            let done = st.inputs.contains_key(&input_final(&r.n));
            let mut by_phase: BTreeMap<String, u64> = BTreeMap::new();
            let (mut requested, mut pending, mut running, mut cpu_ms) = (0u64, 0u64, 0u64, 0u64);
            for (item, it) in Self::items_of(st, node, KIND_SOLVE) {
                *by_phase.entry(it.phase.tag().to_string()).or_default() += 1;
                let w = it.weight;
                requested += w;
                if !Self::item_final(&it.phase) {
                    pending += w;
                }
                for j in &it.jobs {
                    if let Some(jr) = st.jobs.get(j) {
                        cpu_ms += jr.cpu_ms;
                        if matches!(jr.state, AttemptState::Launching | AttemptState::Running) {
                            running += 1;
                        }
                    }
                }
                let _ = item;
            }
            let decided = requested - pending;
            let per_key_s = if decided > 0 {
                Some(cpu_ms as f64 / 1000.0 / decided as f64)
            } else {
                None
            };
            let eta_s = per_key_s.map(|c| c * pending as f64 / running.max(1) as f64);
            let phase = match (&r.running, done, st.nodes.get(node).map(|x| x.state.phase)) {
                // committed in the same tick as this write (the last poll before the campaign finishes)
                _ if finalized.get(&i).is_some_and(|v| v == "closed") => "closed",
                _ if finalized.contains_key(&i) => "final",
                (_, true, Some(NodePhase::Closed | NodePhase::Certified)) => "closed",
                (_, true, _) => "final",
                (Some(p), _, _) if p.round <= 1 => "import",
                (Some(_), _, _) => "checking",
                _ if pending > 0 => "solving",
                _ => "waiting",
            };
            let lc = r.last_check.clone().unwrap_or(Value::Null);
            let answers = self.answers(st, i).len();
            // lane closeloop2: proof / fact items and their jobs
            let mut proof_by_phase: BTreeMap<String, u64> = BTreeMap::new();
            let (mut proof_running, mut proof_cpu_ms) = (0u64, 0u64);
            for it in st.items.values().filter(|it| {
                it.node == *node && (it.kind == KIND_PROOF || it.kind == KIND_FACTS)
            }) {
                *proof_by_phase
                    .entry(format!("{}:{}", it.kind, it.phase.tag()))
                    .or_default() += 1;
                for j in &it.jobs {
                    if let Some(jr) = st.jobs.get(j) {
                        proof_cpu_ms += jr.cpu_ms;
                        if matches!(jr.state, AttemptState::Launching | AttemptState::Running) {
                            proof_running += 1;
                        }
                    }
                }
            }
            nodes.push(json!({
                "node": node, "phase": phase, "round": Self::rounds_done(st, &r.n),
                "keys": {"closure": lc["keys"], "certified": lc["certified"], "open": lc["open"],
                         "requested": requested, "pending": pending},
                "solve_items_by_phase": by_phase, "answers_files": answers,
                "jobs": {"running": running}, "cpu_s": cpu_ms as f64 / 1000.0,
                "per_key_cpu_s": per_key_s, "eta_s": eta_s,
                "pass_running_s": r.pass_started.filter(|_| r.running.is_some()).map(|t| t.elapsed().as_secs()),
                "last_check": {"defect": lc["defect"], "blocking": lc["blocking"], "closed": lc["closed"]},
                "evidence_by_class": lc["evidence_by_class"], "relation_rule": r.cfg.solve.relation_rule,
                "target": lc["target"], "tier": lc["tier"], "exact_tier": lc["exact_tier"],
                "by_rule_class_level": lc["by_rule_class_level"], "relations": lc["relations"],
                "proof_items_by_phase": proof_by_phase,
                "proof_jobs": {"running": proof_running, "cpu_s": proof_cpu_ms as f64 / 1000.0},
            }));
        }
        let v = json!({"schema": PROGRESS_SCHEMA, "ledger_seq": st.last_seq, "nodes": nodes});
        let text = serde_json::to_string_pretty(&v).expect("json");
        let due = match &self.last_progress {
            None => true,
            Some((t, old)) => {
                *old != text && (t.elapsed().as_millis() >= 1000 || !finalized.is_empty())
            }
        };
        if due {
            let p = self.cdir.join("progress").join("closeloop.json");
            if let Some(d) = p.parent() {
                let _ = std::fs::create_dir_all(d);
            }
            let _ = crate::atomic::write_atomic(&p, text.as_bytes());
            self.last_progress = Some((Instant::now(), text));
        }
    }
}

impl CloseFromMaterialSource {
    /// A job without a decision: rerun (ready) until max_job_attempts, then attention (its keys stay open).
    fn fail_or_attention(
        &self,
        st: &CampaignState,
        i: usize,
        done: &Settled,
        code: &str,
        what: &str,
        rt: &mut Routing,
    ) {
        let Some(it) = st.items.get(&done.item) else {
            return;
        };
        let c = &done.classification;
        let failed_before = it
            .jobs
            .iter()
            .filter(|j| *j != &done.job)
            .filter(|j| {
                matches!(
                    st.jobs.get(*j).map(|r| r.state),
                    Some(AttemptState::Failed(_) | AttemptState::BudgetStopped(_))
                )
            })
            .count() as u32;
        if failed_before + 1 >= self.nodes[i].cfg.budgets.max_job_attempts {
            rt.events.push(Event::Item {
                item: done.item.clone(),
                ev: ItemEvent::Attention {
                    code: code.into(),
                    text: format!(
                        "row {}{} ({}): {}; job {} attempt {}; {what}",
                        c.row,
                        if c.sub.is_empty() {
                            String::new()
                        } else {
                            format!("/{}", c.sub)
                        },
                        crate::state::class_word(c.kind),
                        c.why,
                        done.job,
                        done.attempt,
                    ),
                },
            });
        }
    }

    /// lane closeloop2: the point-record table of the certified relations of one settled solve attempt.
    fn relation_table(
        &self,
        i: usize,
        b: &BatchFile,
        item: &str,
        job: &str,
        records: &[(Vec<i32>, Value)],
    ) -> Result<AcceptedRecord, String> {
        let r = &self.nodes[i];
        let recipe: Value = std::fs::read(&b.recipe_path)
            .ok()
            .and_then(|x| serde_json::from_slice(&x).ok())
            .unwrap_or(Value::Null);
        let group = r.proof_inputs.contains_key("symmetry_group");
        let sym = recipe["symmetry_rows"] == true && group;
        let mut text = proofs::relation_header(&r.cfg.node, item, job, &b.source);
        text.push('\n');
        for (idx, (key, rec)) in records.iter().enumerate() {
            let chart = proofs::chart_of(&recipe, key);
            text.push_str(&proofs::relation_record(idx, key, rec, &chart, sym)?);
            text.push('\n');
        }
        Ok(record(
            format!("{}/relations/{item}.{job}.table.jsonl", node_dir(&r.n)),
            text.into_bytes(),
        ))
    }

    /// lane closeloop2: a settled proof / fact job: its verified answers bound again to the batch, written as the
    /// node's evidence file (compact proof / fact verdict lines), recorded; no decision -> rerun, then attention.
    fn route_proof(
        &mut self,
        st: &CampaignState,
        i: usize,
        done: &Settled,
    ) -> Result<Routing, String> {
        let b = self.pbatch(st, i, &done.item)?;
        let c = &done.classification;
        let mut rt = Routing::default();
        if !(c.row == 1 && c.sub.is_empty() && !done.canonical_lines.is_empty() && done.table.is_some()) {
            let what = format!("{} {} keys stay below their level", b.pairs().len(), b.kind);
            self.fail_or_attention(st, i, done, "proof_job_failed", &what, &mut rt);
            return Ok(rt);
        }
        let table = done.table.clone().expect("checked");
        let lines = match proofs::evidence_lines(&b, &done.canonical_lines) {
            Ok(l) => l,
            Err(e) => {
                // the sealed answers do not bind to the batch the ledger names: never recorded
                rt.events.push(Event::Item {
                    item: done.item.clone(),
                    ev: ItemEvent::Attention {
                        code: "proof_answers_refused".into(),
                        text: format!("job {}: {e}", done.job),
                    },
                });
                return Ok(rt);
            }
        };
        let n = self.nodes[i].n.clone();
        let (sub, input, role) = if b.kind == "facts" {
            ("facts", input_facts(&done.item, &done.job), ROLE_FACTS)
        } else {
            ("proofs", input_proofs(&done.item, &done.job), ROLE_PROOFS)
        };
        let mut body = lines.join("\n").into_bytes();
        body.push(b'\n');
        let ans = record(
            format!("{}/{sub}/{}.{}.jsonl", node_dir(&n), done.item, done.job),
            body,
        );
        let aref = ans.file_ref();
        rt.events.push(Event::InputRegistered {
            input,
            role: role.into(),
            path: self.abs(&ans.path).display().to_string(),
            bytes: aref.bytes,
            blake3: aref.blake3.clone(),
        });
        let keys = record(
            format!("{}/{sub}/{}.{}.keys.json", node_dir(&n), done.item, done.job),
            json_bytes(&json!({"schema": "rustred.campaign.closeloop-keys.v1",
                               "pairs": b.pairs().into_iter().collect::<Vec<_>>()})),
        );
        let kref = keys.file_ref();
        let canon = done
            .canonical_blake3
            .clone()
            .unwrap_or_else(|| aref.blake3.clone());
        let (round, _) = parse_proof_item(&n, &done.item).unwrap_or((0, 0));
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
                    proofs: aref,
                    keys: kref,
                }),
                round: Some(round),
            },
        });
        rt.accepts.push(keys);
        rt.accepts.push(ans);
        Ok(rt)
    }
}

impl WorkSource for CloseFromMaterialSource {
    fn bootstrap(
        &mut self,
        st: &CampaignState,
        _ctx: &SourceContext,
    ) -> Result<Vec<Event>, String> {
        let mut ev = Vec::new();
        for r in &self.nodes {
            let n = &r.n;
            let node = &r.cfg.node;
            // the effective import spec (written once; its digest pins the node's material)
            let p = self.abs(&r.spec_rel);
            if std::fs::read(&p).ok().as_deref() != Some(r.spec_text.as_bytes()) {
                if let Some(d) = p.parent() {
                    std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
                }
                crate::atomic::write_atomic(&p, r.spec_text.as_bytes())
                    .map_err(|e| format!("{}: {e}", p.display()))?;
            }
            let digest = blake3_hex(r.spec_text.as_bytes());
            let sid = input_import_spec(n);
            match st.inputs.get(&sid) {
                Some(x) if x.blake3 != digest => {
                    return Err(format!(
                        "node {node}: the effective import spec changed since the campaign started (material, default recipe, required evidence or solve binary): refused; start a new campaign directory"
                    ));
                }
                Some(_) => {}
                None => ev.push(Event::InputRegistered {
                    input: sid.clone(),
                    role: ROLE_SPEC.into(),
                    path: p.display().to_string(),
                    bytes: r.spec_text.len() as u64,
                    blake3: digest,
                }),
            }
            let boxed = r.cfg.target == Target::Boxed;
            if boxed {
                let v: Value = serde_json::from_str(&r.spec_text).map_err(|e| e.to_string())?;
                let sp = v["start"]["path"]
                    .as_str()
                    .ok_or("import spec: start.path missing")?;
                let sp = resolve(&r.cfg.base(), sp);
                let b =
                    std::fs::read(&sp).map_err(|e| format!("start set {}: {e}", sp.display()))?;
                let d = blake3_hex(&b);
                let bid = input_box(n);
                match st.inputs.get(&bid) {
                    Some(x) if x.blake3 != d => {
                        return Err(format!(
                            "node {node}: the start set (box) {} changed since the campaign started: refused",
                            sp.display()
                        ));
                    }
                    Some(_) => {}
                    None => ev.push(Event::InputRegistered {
                        input: bid,
                        role: ROLE_BOX.into(),
                        path: sp.display().to_string(),
                        bytes: b.len() as u64,
                        blake3: d,
                    }),
                }
            }
            match st.nodes.get(node) {
                None => {
                    ev.push(Event::NodeRegistered {
                        node: node.clone(),
                        inputs: vec![self.cfg_input.clone(), sid.clone()],
                        terminal_bound: None,
                        spec_box: boxed.then(|| input_box(n)),
                        spec: NodeSpecValues {
                            full_attempt_cpu_hours: Some(0),
                            full_attempt_fixer_rounds: Some(0),
                            ..NodeSpecValues::default()
                        },
                        reference: None,
                    });
                    ev.push(Event::Node {
                        node: node.clone(),
                        ev: NodeEvent::Start,
                    });
                }
                Some(x) if x.state.phase == NodePhase::Stale => {
                    return Err(format!(
                        "node {node} is stale ({}): the close-from-material source does not redo a node; start a new campaign directory",
                        x.stale_reason.clone().unwrap_or_default()
                    ));
                }
                Some(x) if x.state.phase == NodePhase::Registered => ev.push(Event::Node {
                    node: node.clone(),
                    ev: NodeEvent::Start,
                }),
                Some(_) => {}
            }
            let imp = import_item(n);
            if !st.items.contains_key(&imp) {
                ev.push(Event::ItemCreated {
                    item: imp,
                    node: node.clone(),
                    kind: KIND_IMPORT.into(),
                    rung: Rung(0),
                    parent: None,
                    inputs: vec![sid],
                    label: "import".into(),
                    weight: 1,
                });
            }
        }
        Ok(ev)
    }

    fn ready(&mut self, st: &CampaignState, _ctx: &SourceContext, limit: usize) -> Vec<JobRequest> {
        let mut out = Vec::new();
        for i in 0..self.nodes.len() {
            let node = self.nodes[i].cfg.node.clone();
            let max_att = self.nodes[i].cfg.budgets.max_job_attempts;
            let items: Vec<(String, String, ItemPhase, Vec<String>)> = st
                .items
                .iter()
                .filter(|(_, it)| {
                    it.node == node
                        && (it.kind == KIND_SOLVE || it.kind == KIND_PROOF || it.kind == KIND_FACTS)
                })
                .map(|(k, it)| (k.clone(), it.kind.clone(), it.phase.clone(), it.jobs.clone()))
                .collect();
            for (item, ikind, ph, jobs) in items {
                let (k, j) = parse_solve_item(&self.nodes[i].n, &item)
                    .or_else(|| parse_proof_item(&self.nodes[i].n, &item))
                    .unwrap_or((0, 0));
                let base = -(k as i64) * 1_000_000 - j as i64;
                let failed = jobs
                    .iter()
                    .filter(|x| {
                        matches!(
                            st.jobs.get(*x).map(|r| r.state),
                            Some(AttemptState::Failed(_) | AttemptState::BudgetStopped(_))
                        )
                    })
                    .count() as u32;
                let job = match ph {
                    ItemPhase::Open => Some(format!("{item}-t{}", jobs.len())),
                    ItemPhase::InProgress => {
                        match jobs.last().and_then(|l| st.jobs.get(l)).map(|r| r.state) {
                            Some(AttemptState::Queued) => jobs.last().cloned(),
                            Some(AttemptState::Failed(_) | AttemptState::BudgetStopped(_))
                                if failed < max_att =>
                            {
                                Some(format!("{item}-t{}", jobs.len()))
                            }
                            _ => None,
                        }
                    }
                    _ => None,
                };
                if let Some(job) = job {
                    let req = if ikind == KIND_SOLVE {
                        self.request(st, i, &item, job, base)
                    } else {
                        self.request_proof(st, i, &item, job, base)
                    };
                    match req {
                        Ok(r) => out.push(r),
                        Err(e) => eprintln!("closeloop: no job for {item}: {e}"),
                    }
                }
            }
        }
        out.sort_by(|a, b| b.priority.cmp(&a.priority).then(a.job.cmp(&b.job)));
        out.truncate(limit);
        out
    }

    fn route(
        &mut self,
        st: &CampaignState,
        _ctx: &SourceContext,
        done: &Settled,
    ) -> Result<Routing, String> {
        let it = st
            .items
            .get(&done.item)
            .ok_or_else(|| format!("unknown item {}", done.item))?;
        let i = self
            .node_of(&it.node)
            .ok_or_else(|| format!("item {} of a node this source does not hold", done.item))?;
        if it.kind == KIND_PROOF || it.kind == KIND_FACTS {
            return self.route_proof(st, i, done);
        }
        if it.kind != KIND_SOLVE {
            return Err(format!("item {} is not a solve item", done.item));
        }
        let b = self.batch(st, i, &done.item)?;
        let c = &done.classification;
        let decided_row = matches!((c.row, c.sub.as_str()), (1, "") | (28, "") | (29, ""));
        let mut rt = Routing::default();
        if !(decided_row && !done.canonical_lines.is_empty() && done.table.is_some()) {
            // no per-key decision: rerun (ready) until max_job_attempts, then attention
            let what = format!("{} keys of recipe {} stay open", b.keys.len(), b.source);
            self.fail_or_attention(st, i, done, "solve_job_failed", &what, &mut rt);
            return Ok(rt);
        }
        let table = done.table.clone().expect("checked");
        let set = self.ensure_set(i)?;
        let slot = set
            .recipes
            .iter()
            .find(|r| r.id == b.source)
            .map(|r| r.slot.clone())
            .ok_or_else(|| format!("recipe source {} not in the imported set", b.source))?;
        let skey = format!("{slot:?}");
        if !self.nodes[i].prefixes.contains_key(&skey) {
            let d = prefix_dispatcher(&set, &slot)?;
            self.nodes[i].prefixes.insert(skey.clone(), d);
        }
        let pinned = self.pinned(st, i, &b);
        let prog = self.solve_program(&self.nodes[i].cfg).to_string();
        let r = &self.nodes[i];
        let ctx = AcceptCtx {
            set: &set,
            before: r.prefixes.get(&skey).and_then(|x| x.as_ref()),
            boxed: r.cfg.target == Target::Boxed,
            batch: &b,
            pinned_digest: pinned,
            determinism_id: r.cfg.solve.determinism_id.clone(),
            binary_blake3: self.blake3.get(&prog).cloned().unwrap_or_default(),
            binary_sha256: self.sha256.get(&prog).cloned().unwrap_or_default(),
            binary_path: self
                .programs
                .get(&prog)
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
            family_path: r.cfg.solve.family.display().to_string(),
            family_blake3: r.family_blake3.clone(),
            order: r.cfg.solve.order_arg(),
            results_path: table.path.clone(),
            results_blake3: table.blake3.clone(),
            run_dir: st
                .jobs
                .get(&done.job)
                .and_then(|j| j.run_dir.clone())
                .unwrap_or_default(),
        };
        let d = decide(&ctx, &done.canonical_lines)?;
        if let (None, Some(e)) = (self.nodes[i].pins.get(&b.source), &d.engine_digest)
            && d.refused.is_none()
        {
            self.nodes[i].pins.insert(b.source.clone(), e.clone());
        }
        let n = self.nodes[i].n.clone();
        let dir = format!("{}/answers", node_dir(&n));
        let mut ab = d.lines.join("\n").into_bytes();
        ab.push(b'\n');
        let ans = record(format!("{dir}/{}.{}.jsonl", done.item, done.job), ab);
        let aref = ans.file_ref();
        rt.events.push(Event::InputRegistered {
            input: input_answers(&done.item, &done.job),
            role: ROLE_ANSWERS.into(),
            path: self.abs(&ans.path).display().to_string(),
            bytes: aref.bytes,
            blake3: aref.blake3.clone(),
        });
        let (round, _) = parse_solve_item(&n, &done.item).unwrap_or((0, 0));
        if !d.records.is_empty() {
            // lane closeloop2 (closeloop.md 13.1): our relations as POINT RECORDS, proved per key by proof jobs
            let rel = self.relation_table(i, &b, &done.item, &done.job, &d.records)?;
            let rref = rel.file_ref();
            rt.events.push(Event::InputRegistered {
                input: input_relations(&done.item, &done.job),
                role: ROLE_RELATIONS.into(),
                path: self.abs(&rel.path).display().to_string(),
                bytes: rref.bytes,
                blake3: rref.blake3,
            });
            rt.accepts.push(rel);
        }
        if !d.certified.is_empty() {
            let keys = record(
                format!("{dir}/{}.{}.keys.json", done.item, done.job),
                json_bytes(
                    &json!({"schema": "rustred.campaign.closeloop-keys.v1", "keys": d.certified}),
                ),
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
                    round: Some(round),
                },
            });
            rt.accepts.push(keys);
        } else {
            rt.events.push(Event::Item {
                item: done.item.clone(),
                ev: ItemEvent::Escalate {
                    to: Rung(1),
                    reason: match &d.refused {
                        Some(w) => format!("attempt refused: {w}; the keys go to the next recipe candidate"),
                        None => format!(
                            "no key certified ({} failures); the keys go to the next recipe candidate",
                            d.failed.len()
                        ),
                    },
                },
            });
        }
        rt.accepts.push(ans);
        Ok(rt)
    }

    fn control(
        &mut self,
        st: &CampaignState,
        verb: &str,
        target: &str,
    ) -> Result<Vec<Event>, String> {
        let it = st
            .items
            .get(target)
            .ok_or_else(|| format!("unknown item {target}"))?;
        if self.node_of(&it.node).is_none() {
            return Err(format!("item {target} is not this source's"));
        }
        match verb {
            "retry" | "skip" => Ok(vec![]),
            v => Err(format!("verb {v} not supported by the closeloop source")),
        }
    }

    fn poll(&mut self, st: &CampaignState, _ctx: &SourceContext) -> Result<Routing, String> {
        let mut rt = Routing::default();
        let mut finalized: BTreeMap<usize, String> = BTreeMap::new();
        for i in 0..self.nodes.len() {
            let fin = self.nodes[i]
                .running
                .as_ref()
                .is_some_and(|r| r.handle.is_finished());
            if fin {
                let run = self.nodes[i].running.take().expect("running");
                let round = run.round;
                let res = run
                    .handle
                    .join()
                    .unwrap_or_else(|_| Err("pass thread panicked".into()));
                match res {
                    Ok(PassResult::Loaded(s)) => self.nodes[i].set = Some(s),
                    Ok(PassResult::Checked(o)) => {
                        // the pass ran on the state of its start: still the node's next round?
                        if self.needs_pass(st, i) == Some(o.round) {
                            if let Some(f) = &o.final_ {
                                finalized.insert(i, f.verdict.clone());
                            }
                            match self.commit_pass(st, i, *o) {
                                Ok(r) => {
                                    rt.events.extend(r.events);
                                    rt.accepts.extend(r.accepts);
                                }
                                Err(e) => {
                                    finalized.insert(i, "error".into());
                                    let r = self.error_final(st, i, round, &e);
                                    rt.events.extend(r.events);
                                    rt.accepts.extend(r.accepts);
                                }
                            }
                        } else {
                            self.nodes[i].set = Some(o.set);
                        }
                    }
                    Err(e) => {
                        let r = self.error_final(st, i, round, &e);
                        rt.events.extend(r.events);
                        rt.accepts.extend(r.accepts);
                    }
                }
                continue;
            }
            if let Some(k) = self.needs_pass(st, i)
                && let Err(e) = self.start_pass(st, i, k)
            {
                let r = self.error_final(st, i, k, &e);
                rt.events.extend(r.events);
                rt.accepts.extend(r.accepts);
            }
        }
        self.progress(st, &finalized);
        Ok(rt)
    }

    fn finished(&self, st: &CampaignState) -> bool {
        self.nodes
            .iter()
            .all(|r| r.running.is_none() && st.inputs.contains_key(&input_final(&r.n)))
    }

    fn describe(&self) -> String {
        format!(
            "close from material: {} node(s) [{}]",
            self.nodes.len(),
            self.nodes
                .iter()
                .map(|r| r.cfg.node.clone())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}
