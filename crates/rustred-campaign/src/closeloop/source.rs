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
    AcceptedRecord, CampaignOutcome, JobRequest, Routing, Settled, SourceContext, WorkSource,
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
/// lane t266run: the typed refusal of a set whose re-derived effective spec differs from the one its pass recorded.
pub const EFFECTIVE_SPEC_MISMATCH: &str = "effective_spec_mismatch";
/// lane t266run: an item whose settled attempt could not be routed (requeued as a new attempt, then attention).
pub const ROUTING_REFUSED: &str = "routing_refused";

/// Fake-child answers per (recipe source, key): verdict and right-hand-side keys (tests only).
type Oracle = BTreeMap<(String, Vec<i32>), (Verdict, Vec<Vec<i64>>)>;
/// Issued (recipe source, key) pairs, keys issued, jobs issued, items in attention, and (lane closeloop3) the
/// ERROR / IRREGULAR keys: pairs re-asked now (retry), pairs in attention (attempts used up).
type Issued = (BTreeSet<(String, Vec<i32>)>, u64, u64, u64, u64, Vec<Value>);

/// lane closeloop2: (record, key) pairs given to proof (span) items and to fact items, proof jobs issued, proof items
/// in attention.
#[derive(Default)]
struct ProofIssued {
    spans: BTreeSet<(String, Vec<i32>)>,
    facts: BTreeSet<(String, Vec<i32>)>,
    /// lane closeloop4: pairs given to a SECOND-route fact item (reason second_route).
    facts2: BTreeSet<(String, Vec<i32>)>,
    /// lane zeros2: pairs given to a zeros item (reason all_zeros*).
    zeros2: BTreeSet<(String, Vec<i32>)>,
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
    /// lane layers (closeloop.md 23): records of layers the running pass froze (registered while it runs)
    layer_rx: Option<std::sync::mpsc::Receiver<super::layers::LayerMsg>>,
    /// lane t266run: BLAKE3 of the effective import spec text the pass loads / checks.
    digest: Option<String>,
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
    /// lane closeloop3: TESTS ONLY, lies of the fake `coefficient-facts` child (keyed by key).
    exact_oracle: ProofOracle,
    /// BLAKE3 of the proof config's family / cert / symmetry group files.
    proof_inputs: BTreeMap<String, (PathBuf, String)>,
    /// lane t266run: BLAKE3 of the effective import spec text the cached set was loaded from, and the chart text.
    set_digest: Option<String>,
    chart: Option<String>,
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

// ------------------------------------------------------------------------------------------------------------------
// lane closeloop3 (closeloop.md 16.7): the proof pass for PHASE-E (termmin) relations. Phase E (`termmin::TermminSource`)
// records its certified relations as point-record tables (role `closeloop_relations`, like the closing loop's own);
// these helpers give each record a span proof item of THIS source (the same jobs, settle path and evidence files as
// closeloop.md 13.1 (b)), tell phase E when they are all final, and compute the import's `recipe_proofs` from them.
impl CloseFromMaterialSource {
    /// The relation point-record table of a phase-E solve; its header's `recipe_source` is `source` (the recipe id the
    /// extended import spec gives the relation). None when the node runs no relation proofs.
    pub(crate) fn phase_e_relation_table(
        &self,
        node: &str,
        b: &BatchFile,
        source: &str,
        item: &str,
        job: &str,
        records: &[(Vec<i32>, Value)],
    ) -> Result<Option<AcceptedRecord>, String> {
        let Some(i) = self.node_of(node) else {
            return Ok(None);
        };
        if self.nodes[i]
            .cfg
            .proof
            .as_ref()
            .is_none_or(|p| !p.relations)
        {
            return Ok(None);
        }
        let mut b2 = b.clone();
        b2.source = source.to_string();
        self.relation_table(i, &b2, item, job, records).map(Some)
    }

    /// Span proof items for the records of the node's relation tables registered by items of kind `kind` that no
    /// proof item asked yet (one item per record, reason `relation`; ids `<n>.p0.<j>`: round 0 is never a pass).
    pub(crate) fn phase_e_proof_items(
        &mut self,
        st: &CampaignState,
        node: &str,
        kind: &str,
    ) -> Result<Routing, String> {
        let mut rt = Routing::default();
        let Some(i) = self.node_of(node) else {
            return Ok(rt);
        };
        if self.nodes[i]
            .cfg
            .proof
            .as_ref()
            .is_none_or(|p| !p.relations)
        {
            return Ok(rt);
        }
        let asked = self.proof_issued(st, i)?.spans;
        let n = self.nodes[i].n.clone();
        let prefix = format!("{n}.p0.");
        let mut j = st
            .items
            .keys()
            .filter_map(|k| {
                k.strip_prefix(&prefix)
                    .and_then(|x| x.parse::<usize>().ok())
            })
            .map(|x| x + 1)
            .max()
            .unwrap_or(0);
        for (id, p, _) in self.inputs_of_role(st, i, ROLE_RELATIONS) {
            let item = id
                .strip_prefix("closeloop:")
                .and_then(|x| x.split_once(':'))
                .map(|(it, _)| it.to_string())
                .unwrap_or_default();
            if st.items.get(&item).is_none_or(|it| it.kind != kind) {
                continue;
            }
            let text = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
            let source = text
                .split(|&c| c == b'\n')
                .next()
                .and_then(|h| serde_json::from_slice::<Value>(h).ok())
                .and_then(|h| h["recipe_source"].as_str().map(str::to_string))
                .unwrap_or_default();
            for (rule, offset, sha, key) in proofs::relation_records(&text) {
                if asked.contains(&(sha.clone(), key.clone())) {
                    continue;
                }
                let b = ProofBatch {
                    kind: "span".into(),
                    reason: proofs::REASON_RELATION.into(),
                    table: p.display().to_string(),
                    records: vec![proofs::RecordReq {
                        rule,
                        offset,
                        record_sha256: sha,
                        keys: vec![key],
                        ..Default::default()
                    }],
                    relation_source: Some(source.clone()),
                };
                self.new_proof_item(&mut rt, i, 0, j, &b)?;
                j += 1;
            }
        }
        Ok(rt)
    }

    /// Whether any proof / fact item of the node is not final.
    pub(crate) fn proofs_pending(&self, st: &CampaignState, node: &str) -> bool {
        st.items
            .values()
            .any(|it| it.node == node && is_proof_kind(&it.kind) && !Self::item_final(&it.phase))
    }

    /// (recipe source, key) of every relation record of the node (any `closeloop_relations` table: the closing loop's
    /// and phase E's) with an exact per-key proof bound (EXACT_IN_SPAN_AT_KEY to (record_sha256, key)): the import's
    /// `Evidence::recipe_proofs` (closeloop.md 13.6).
    pub(crate) fn proved_relations(
        &self,
        st: &CampaignState,
        node: &str,
    ) -> std::collections::HashSet<(String, Vec<i32>)> {
        let mut out = std::collections::HashSet::new();
        let Some(i) = self.node_of(node) else {
            return out;
        };
        let mut by_sha: BTreeMap<String, (String, Vec<i32>)> = BTreeMap::new();
        for (_, p, _) in self.inputs_of_role(st, i, ROLE_RELATIONS) {
            let Ok(text) = std::fs::read(&p) else {
                continue;
            };
            let source = text
                .split(|&c| c == b'\n')
                .next()
                .and_then(|h| serde_json::from_slice::<Value>(h).ok())
                .and_then(|h| h["recipe_source"].as_str().map(str::to_string))
                .unwrap_or_default();
            for (_, _, sha, key) in proofs::relation_records(&text) {
                by_sha.insert(sha, (source.clone(), key));
            }
        }
        for (_, p, _) in self.inputs_of_role(st, i, ROLE_PROOFS) {
            for l in std::fs::read_to_string(&p).unwrap_or_default().lines() {
                let Ok(v) = serde_json::from_str::<Value>(l) else {
                    continue;
                };
                let (Some(sha), Ok(key)) = (
                    v["record_sha256"].as_str(),
                    serde_json::from_value::<Vec<i32>>(v["key"].clone()),
                ) else {
                    continue;
                };
                if v["verdict"] == "EXACT_IN_SPAN_AT_KEY"
                    && let Some((s, k)) = by_sha.get(sha)
                    && *k == key
                {
                    out.insert((s.clone(), key));
                }
            }
        }
        out
    }
}

/// lane closeloop2 / closeloop3: the item kinds of proof jobs (span, rr_proof facts, exact-eval facts).
fn is_proof_kind(k: &str) -> bool {
    k == KIND_PROOF || k == KIND_FACTS || k == KIND_EXACT
}

/// lane t266run (L17): the attention code of an attempt without a decision: a child killed at its MEMORY CAP is a
/// resource end (`memory_cap_exceeded`: retried, then attention; a session with a higher `--mem-cap` reopens the item),
/// never evidence about the keys; anything else keeps the caller's code.
fn failure_code<'a>(done: &Settled, otherwise: &'a str) -> &'a str {
    if done.classification.kind == crate::state::FinishClass::KilledMemory {
        crate::launch::MEMORY_CAP_EXCEEDED
    } else {
        otherwise
    }
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

use super::proofs::file_blake3_cached;

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
            let mut exact_oracle = BTreeMap::new();
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
                // lane closeloop3: the exact-evaluation program (its digests are recorded like every program's)
                if let Some(x) = &pc.exact_eval {
                    let prog = x.program_key().to_string();
                    let path = s.programs.get(&prog).cloned().ok_or_else(|| {
                        format!(
                            "node {}: exact_eval program {prog} not in the spec",
                            nc.node
                        )
                    })?;
                    s.sha256.insert(prog.clone(), file_sha256(&path)?);
                    s.blake3.insert(prog, file_blake3(&path)?);
                    if let Some(o) = x.fake.as_ref().and_then(|f| f.oracle.as_ref()) {
                        exact_oracle = proofs::read_oracle(o)?;
                    }
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
                exact_oracle,
                proof_inputs,
                set_digest: None,
                chart: None,
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

    /// A recorded input path: absolute as recorded, else relative to the campaign directory.
    fn abs_or(&self, p: &str) -> PathBuf {
        let pb = PathBuf::from(p);
        if pb.is_absolute() {
            pb
        } else {
            self.cdir.join(pb)
        }
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

    /// lane t266run (L17): an item in attention for a memory-cap end (it holds its node until the cap is raised).
    fn memory_cap_held(st: &CampaignState, item: &str) -> bool {
        crate::launch::ended_at_memory_cap(st, item)
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

    /// lane t266run: NODE-level inputs `closeloop:<n>:<what>` of role `role`, in ledger order.
    /// The node's inputs of `role` in registration order; with `before = Some(k)` only those registered by the commits
    /// of rounds < k (input ids `closeloop:<n>:<what>:r<round>`; an id without a round suffix never passes a bound).
    fn node_inputs_of_role(
        &self,
        st: &CampaignState,
        i: usize,
        role: &str,
        before: Option<u32>,
    ) -> Vec<PathBuf> {
        let pre = format!("closeloop:{}:", self.nodes[i].n);
        let mut v: Vec<_> = st
            .inputs
            .iter()
            .filter(|(id, r)| {
                r.role == role && id.starts_with(&pre) && Self::round_before(id, before)
            })
            .map(|(_, r)| (r.registered_seq, PathBuf::from(&r.path), r.blake3.clone()))
            .collect();
        v.sort();
        v.into_iter().map(|(_, p, _)| p).collect()
    }

    /// Whether the input id's round suffix (`:r<k>`) is below `before` (always true without a bound).
    fn round_before(id: &str, before: Option<u32>) -> bool {
        let Some(b) = before else {
            return true;
        };
        id.rsplit_once(":r")
            .and_then(|(_, k)| k.parse::<u32>().ok())
            .is_some_and(|k| k < b)
    }

    /// lane t266run: the escalation recorded for node `i` (closeloop.md 17); every file is checked against its
    /// recorded BLAKE3 before it is read. `before = Some(k)`: only what the commits of rounds < k recorded (= the
    /// escalation the pass of round k ran on).
    fn derived_state(
        &self,
        st: &CampaignState,
        i: usize,
        before: Option<u32>,
    ) -> Result<super::derived::DerivedState, String> {
        use super::derived::{ROLE_RECIPES, ROLE_SYMREL, ROLE_TERMINALS};
        let pre = format!("closeloop:{}:", self.nodes[i].n);
        for (id, r) in &st.inputs {
            if id.starts_with(&pre)
                && [ROLE_SYMREL, ROLE_RECIPES, ROLE_TERMINALS].contains(&r.role.as_str())
                && Self::round_before(id, before)
            {
                let got = file_blake3(Path::new(&r.path))?;
                if got != r.blake3 {
                    return Err(format!(
                        "{}: BLAKE3 {got} differs from the recorded {}",
                        r.path, r.blake3
                    ));
                }
            }
        }
        super::derived::DerivedState::read(
            &self.node_inputs_of_role(st, i, ROLE_SYMREL, before),
            &self.node_inputs_of_role(st, i, ROLE_RECIPES, before),
            &self.node_inputs_of_role(st, i, ROLE_TERMINALS, before),
        )
    }

    /// The chart (line mask) of node `i`: the loaded set's, else read from the frame header of the base import spec
    /// (no table is read).
    fn chart_of(&self, i: usize) -> Result<String, String> {
        if let Some(c) = &self.nodes[i].chart {
            return Ok(c.clone());
        }
        let r = &self.nodes[i];
        let spec = rustred_campaign_core::import::TableSetSpec::from_json(&r.spec_text)
            .map_err(|e| format!("import spec: {e}"))?;
        let f = rustred_campaign_core::import::load::frame_of(&spec, &r.cfg.base())
            .map_err(|e| format!("frame: {e}"))?;
        Ok(f.mask_string())
    }

    /// lane t266run: the effective import spec the node's LAST pass ran on, for a set loaded after a restart (settles
    /// of the batches that pass issued route against it). Round L = the number of check records; its spec is
    /// RE-DERIVED from the base spec and the escalation inputs of rounds < L (each BLAKE3-verified against the
    /// ledger) and VERIFIED against the spec digest the pass of round L recorded in its check record (itself
    /// BLAKE3-verified): a mismatch is refused, nothing is trusted from a file a previous session wrote. Before the
    /// first pass: the base spec.
    fn last_pass_spec(&mut self, st: &CampaignState, i: usize) -> Result<String, String> {
        let n = self.nodes[i].n.clone();
        let l = Self::rounds_done(st, &n);
        if l == 0 {
            return Ok(self.nodes[i].spec_text.clone());
        }
        let derived = self.derived_state(st, i, Some(l))?;
        let chart = self.chart_of(i)?;
        let eff = {
            let r = &self.nodes[i];
            super::derived::effective_spec(
                &r.spec_text,
                &derived,
                r.cfg.escalation.as_ref(),
                &self.abs(&format!("{}/derived", node_dir(&r.n))),
                &chart,
                super::derived::DerivedFiles::Verify,
            )?
        };
        let check: Value = serde_json::from_slice(&self.input_file(st, &input_check(&n, l))?)
            .map_err(|e| format!("check record of round {l}: {e}"))?;
        let want = check["spec_digest"]
            .as_str()
            .ok_or_else(|| format!("check record of round {l}: no spec_digest"))?;
        let got = rustred_campaign_core::import::TableSetSpec::from_json(&eff)
            .map_err(|e| format!("import spec: {e}"))?
            .digest();
        if got != want {
            return Err(format!(
                "{EFFECTIVE_SPEC_MISMATCH}: the effective import spec of round {l} re-derived from the ledger has digest {got}, the pass of round {l} recorded {want}: refused"
            ));
        }
        self.nodes[i].chart = Some(chart);
        Ok(eff)
    }

    /// (recipe source, key) pairs of every solve item of the node, keys issued, jobs issued, items in attention.
    fn issued(&mut self, st: &CampaignState, i: usize) -> Result<Issued, String> {
        let node = self.nodes[i].cfg.node.clone();
        let items: Vec<(String, ItemPhase)> = Self::items_of(st, &node, KIND_SOLVE)
            .map(|(k, it)| (k.clone(), it.phase.clone()))
            .collect();
        let mut pairs = BTreeSet::new();
        let mut given: BTreeMap<(String, Vec<i32>), u32> = BTreeMap::new();
        let (mut keys, mut att) = (0u64, 0u64);
        for (item, ph) in &items {
            let b = self.batch(st, i, item)?;
            keys += b.keys.len() as u64;
            for k in b.keys {
                *given.entry((b.source.clone(), k.clone())).or_default() += 1;
                pairs.insert((b.source.clone(), k));
            }
            if matches!(ph, ItemPhase::Attention(_)) {
                att += 1;
            }
        }
        // lane closeloop3 (closeloop.md 16.6): a pair whose EVERY solve so far ended ERROR / IRREGULAR is asked again
        // (a later round: the back-off) until max_job_attempts, then it is in attention (never "no relation")
        let mut failed: BTreeMap<(String, Vec<i32>), (u32, String)> = BTreeMap::new();
        for (_, p, _) in self.inputs_of_role(st, i, ROLE_RETRY) {
            let t = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
            for l in t.lines().filter(|l| !l.trim().is_empty()) {
                let v: Value =
                    serde_json::from_str(l).map_err(|e| format!("{}: {e}", p.display()))?;
                let key: Vec<i32> =
                    serde_json::from_value(v["key"].clone()).map_err(|e| e.to_string())?;
                let e = failed
                    .entry((v["source"].as_str().unwrap_or("").to_string(), key))
                    .or_default();
                e.0 += 1;
                e.1 = v["verdict"].as_str().unwrap_or("").to_string();
            }
        }
        let max = self.nodes[i].cfg.budgets.max_job_attempts;
        let (mut retried, mut attention) = (0u64, vec![]);
        for (pair, (n, verdict)) in failed {
            if given.get(&pair).copied().unwrap_or(0) != n {
                continue; // a later solve of the pair is pending or decided it
            }
            if n < max {
                pairs.remove(&pair);
                retried += 1;
            } else {
                attention.push(
                    json!({"source": pair.0, "key": pair.1, "verdict": verdict, "attempts": n}),
                );
            }
        }
        Ok((pairs, keys, items.len() as u64, att, retried, attention))
    }

    fn pbatch(
        &mut self,
        st: &CampaignState,
        i: usize,
        item: &str,
    ) -> Result<ProofBatchFile, String> {
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
            .filter(|(_, it)| it.node == node && (is_proof_kind(&it.kind)))
            .map(|(k, it)| (k.clone(), it.phase.clone()))
            .collect();
        let mut out = ProofIssued::default();
        for (item, ph) in &items {
            let b = self.pbatch(st, i, item)?;
            if b.reason == proofs::REASON_SECOND_ROUTE {
                out.facts2.extend(b.pairs());
            } else if proofs::is_zeros_reason(&b.reason) {
                out.zeros2.extend(b.pairs());
            } else if b.kind == "facts" || b.kind == "exact" {
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
        // lane closeloop4: proposals waiting for the operator: no pass until a decision is recorded
        if self.awaiting(st, i).is_some() {
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
        // lane t266run (L17): an item in attention for a MEMORY-CAP end is not final for the loop: it holds the node
        // (no pass, no final verdict) until a session with a higher --mem-cap reopens it
        let busy = st.items.iter().any(|(id, it)| {
            it.node == *node
                && (it.kind == KIND_SOLVE || is_proof_kind(&it.kind))
                && (!Self::item_final(&it.phase) || Self::memory_cap_held(st, id))
        });
        if busy {
            return if r.set.is_none() { Some(0) } else { None };
        }
        Some(Self::rounds_done(st, &r.n) + 1)
    }

    fn start_pass(&mut self, st: &CampaignState, i: usize, round: u32) -> Result<(), String> {
        let cached = self.nodes[i].set.clone();
        // lane closeloop4 (closeloop.md 19): the node's spec + the decided terminals (then t266run's derived part)
        let decisions = self.terminal_decisions(st, i)?;
        let spec_text = super::terminals::effective_spec(&self.nodes[i].spec_text, &decisions)?;
        if round == 0 {
            // lane t266run: the set of a restarted session is the one the last pass ran on (re-derived + verified;
            // before any pass: the base spec), so the settles of the batches that pass issued route against it
            let text = self.last_pass_spec(st, i)?;
            let r = &self.nodes[i];
            let base = r.cfg.base();
            // lane t266run: a persistent table index in big-node mode (an unchanged table is not re-read)
            let index_dir = r.cfg.scale.as_ref().map(|sc| {
                sc.index_dir
                    .clone()
                    .unwrap_or_else(|| self.abs(&format!("{}/index", node_dir(&r.n))))
            });
            let digest_text = text.clone();
            let h = std::thread::Builder::new()
                .name(format!("closeloop-load-{}", r.n))
                .spawn(move || {
                    let spec = rustred_campaign_core::import::TableSetSpec::from_json(&text)
                        .map_err(|e| format!("import spec: {e}"))?;
                    let set = rustred_campaign_core::import::load_with(
                        &spec,
                        &base,
                        &rustred_campaign_core::import::LoadOptions { index_dir },
                    )
                    .map_err(|e| format!("import load: {e}"))?;
                    Ok(PassResult::Loaded(Arc::new(set)))
                })
                .map_err(|e| format!("pass thread: {e}"))?;
            self.nodes[i].running = Some(Running {
                round,
                handle: h,
                layer_rx: None,
                digest: Some(blake3_hex(digest_text.as_bytes())),
            });
            self.nodes[i].pass_started = Some(Instant::now());
            return Ok(());
        }
        let (issued, keys_issued, jobs_issued, att, engine_retried, engine_attention) =
            self.issued(st, i)?;
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
        let fact2_files = self
            .inputs_of_role(st, i, ROLE_FACTS2)
            .into_iter()
            .map(|(_, p, b)| (p, b))
            .collect();
        let zeros2_files = self
            .inputs_of_role(st, i, ROLE_ZEROS2)
            .into_iter()
            .map(|(_, p, b)| (p, b))
            .collect();
        let relation_tables = self.relation_tables(st, i);
        // lane t266run (closeloop.md 17): the escalation recorded so far and the effective spec it implies; a set
        // loaded from another effective spec is dropped (the pass reloads it; the table index makes that cheap)
        let derived = self.derived_state(st, i, None)?;
        let eff = {
            let r = &self.nodes[i];
            let chart = r.chart.clone().unwrap_or_default();
            super::derived::effective_spec(
                &spec_text,
                &derived,
                r.cfg.escalation.as_ref(),
                &self.abs(&format!("{}/derived", node_dir(&r.n))),
                &chart,
                super::derived::DerivedFiles::Write,
            )?
        };
        let eff_digest = blake3_hex(eff.as_bytes());
        // lane closeloop4: the set of ANOTHER effective spec is not given to the pass (it reloads), but it is KEPT for the
        // acceptance of settled solves (their recipes are in the set that issued them): a settle never loads tables on
        // the supervisor's main thread
        let cached = if self.nodes[i].set_digest.as_deref() == Some(eff_digest.as_str()) {
            cached
        } else {
            None
        };
        // lane closeloop4 (audit-closeloop M1): the files' digests recorded at their first use (ledger)
        let pins_recorded: Option<BTreeMap<String, String>> =
            if st.inputs.contains_key(&input_pins(&self.nodes[i].n)) {
                Some(
                    serde_json::from_slice(&self.input_file(st, &input_pins(&self.nodes[i].n))?)
                        .map_err(|e| format!("pins: {e}"))?,
                )
            } else {
                None
            };
        // lane layers (closeloop.md 23): the frozen layers come back ONLY from the ledger (record BLAKE3 verified here;
        // the pass verifies the sha256 of every file the record pins, the leaf-set digest and the walk binding)
        let mut frozen_layers: Vec<(usize, Vec<u8>)> = vec![];
        {
            let pre = format!("closeloop:{}:layer:", self.nodes[i].n);
            let ids: Vec<(usize, String)> = st
                .inputs
                .iter()
                .filter(|(id, r)| id.starts_with(&pre) && r.role == super::layers::ROLE_LAYER)
                .filter_map(|(id, _)| {
                    id[pre.len()..]
                        .parse::<usize>()
                        .ok()
                        .map(|k| (k, id.clone()))
                })
                .collect();
            for (k, id) in ids {
                frozen_layers.push((k, self.input_file(st, &id)?));
            }
            frozen_layers.sort_by_key(|x| x.0);
        }
        let (layer_tx, layer_rx) = std::sync::mpsc::channel::<super::layers::LayerMsg>();
        let recipe_dir = self.abs(&format!(
            "{}/escalation/r{round}",
            node_dir(&self.nodes[i].n)
        ));
        let r = &self.nodes[i];
        let inp = PassIn {
            node: r.cfg.node.clone(),
            round,
            spec_text: eff,
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
            engine_retried,
            engine_attention,
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
                exact_keys_per_job: p.exact_eval.as_ref().map(|x| x.max_keys_per_job),
                max_fact_terms_per_job: p.max_fact_terms_per_job,
            }),
            exact_tier: r.cfg.exact_tier,
            rows_path: self.abs(&format!("{}/checks/r{round}.rows.jsonl", node_dir(&r.n))),
            target: match r.cfg.target {
                Target::Boxed => "boxed".into(),
                Target::Full => "full".into(),
            },
            box_clause: r.cfg.box_clause.clone().unwrap_or_else(|| "box".into()),
            prior_clauses: r
                .cfg
                .prior_clauses
                .iter()
                .map(|c| (c.id.clone(), c.result.clone()))
                .collect(),
            fact2_files,
            issued_facts2: pi.facts2,
            zeros2_files,
            issued_zeros2: pi.zeros2,
            second_route: r.cfg.second_route,
            terminals: r.cfg.terminals,
            terminal_decisions: decisions,
            pins_recorded,
            reference_mismatch: r.cfg.reference.mismatch.clone(),
            prior_sha256: r
                .cfg
                .prior_clauses
                .iter()
                .filter_map(|c| c.sha256.clone().map(|d| (c.id.clone(), d)))
                .collect(),
            scale: r.cfg.scale.clone(),
            escalation: r.cfg.escalation.clone(),
            derived,
            recipe_dir,
            clauses: r.cfg.clauses.clone(),
            layers: r.cfg.layers.clone(),
            prior_leaf_sets: r
                .cfg
                .prior_clauses
                .iter()
                .filter_map(|c| c.leaf_set.clone().map(|l| (c.id.clone(), l)))
                .collect(),
            frozen_layers,
            layer_tx: Some(layer_tx),
        };
        let h = std::thread::Builder::new()
            .name(format!("closeloop-pass-{}", r.n))
            .spawn(move || run_pass(&inp, cached).map(|o| PassResult::Checked(Box::new(o))))
            .map_err(|e| format!("pass thread: {e}"))?;
        self.nodes[i].running = Some(Running {
            round,
            handle: h,
            layer_rx: Some(layer_rx),
            digest: Some(eff_digest),
        });
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
            rss: if f.rss_bytes > 0 {
                RssPlan {
                    grow_bytes_per_s: f.rss_bytes.saturating_mul(4),
                    max_bytes: f.rss_bytes,
                }
            } else {
                RssPlan::default()
            },
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

    fn ensure_set(&mut self, st: &CampaignState, i: usize) -> Result<Arc<ImportedSet>, String> {
        if let Some(s) = &self.nodes[i].set {
            return Ok(s.clone());
        }
        // only at recovery (a settled attempt routed before any pass ran): synchronous load of the set the LAST pass
        // ran on (lane t266run: re-derived from the ledger and verified against the digest that pass recorded)
        let text = self.last_pass_spec(st, i)?;
        let r = &self.nodes[i];
        let spec = rustred_campaign_core::import::TableSetSpec::from_json(&text)
            .map_err(|e| format!("import spec: {e}"))?;
        // lane t266run: the persistent table index in big-node mode (the tables are what this set is used for here)
        let index_dir = r.cfg.scale.as_ref().map(|sc| {
            sc.index_dir
                .clone()
                .unwrap_or_else(|| self.abs(&format!("{}/index", node_dir(&r.n))))
        });
        let set = Arc::new(
            rustred_campaign_core::import::load_with(
                &spec,
                &r.cfg.base(),
                &rustred_campaign_core::import::LoadOptions { index_dir },
            )
            .map_err(|e| format!("import load: {e}"))?,
        );
        self.nodes[i].set_digest = Some(blake3_hex(text.as_bytes()));
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
                // closeloop.md 13.3 / 16.5: a node registered `start_target: boxed` (lane closeloop3) is BOXED from its
                // first record and never comes here; this pair stays ONLY for nodes of a campaign directory registered
                // before the ledger had a boxed start (resume of an older ledger: FULL at registration)
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
        // lane closeloop4 (audit-closeloop M1): the digests of every file the passes read, registered at first use and
        // whenever a pass read a file not recorded before (an existing entry never changes: the pass refuses that)
        {
            let body = json_bytes(&serde_json::to_value(&out.pins).expect("json"));
            let rec = record(format!("{}/pins.json", node_dir(&n)), body);
            let fr = rec.file_ref();
            if st
                .inputs
                .get(&input_pins(&n))
                .is_none_or(|r| r.blake3 != fr.blake3)
            {
                rt.events.push(Event::InputRegistered {
                    input: input_pins(&n),
                    role: ROLE_PINS.into(),
                    path: self.abs(&rec.path).display().to_string(),
                    bytes: fr.bytes,
                    blake3: fr.blake3,
                });
                rt.accepts.push(rec);
            }
        }
        // lane closeloop4 (closeloop.md 19): terminal proposals waiting for a decision / an automatic decision
        if let Some(p) = &out.terminal_proposals {
            let rec = record(
                format!("{}/final/terminal_proposals.r{k}.json", node_dir(&n)),
                json_bytes(p),
            );
            let fr = rec.file_ref();
            rt.events.push(Event::InputRegistered {
                input: input_tproposals(&n, k),
                role: ROLE_TPROPOSALS.into(),
                path: self.abs(&rec.path).display().to_string(),
                bytes: fr.bytes,
                blake3: fr.blake3,
            });
            rt.events.push(Event::Note {
                text: format!(
                    "closeloop {node}: {} (round {k}); `rr_campaign control CDIR resolve-terminals --node {node} --accept KEYS | --reject`",
                    p["label"].as_str().unwrap_or("terminal proposals")
                ),
            });
            rt.accepts.push(rec);
        }
        if let Some(d) = &out.terminal_decision {
            let (ev, rec) = self.decision_record(i, d)?;
            rt.events.push(ev);
            rt.accepts.push(rec);
        }
        // lane t266run (closeloop.md 17): the escalation's records (symmetry attempts, step recipes, accepted terminals)
        if let Some(e) = &out.escalation {
            use super::derived::{ROLE_RECIPES, ROLE_SYMREL, ROLE_TERMINALS};
            let reg = |rt: &mut Routing, what: &str, role: &str, rel: String, body: Vec<u8>| {
                let rec = record(rel, body);
                let fref = rec.file_ref();
                rt.events.push(Event::InputRegistered {
                    input: format!("closeloop:{n}:{what}:r{k}"),
                    role: role.into(),
                    path: self.abs(&rec.path).display().to_string(),
                    bytes: fref.bytes,
                    blake3: fref.blake3.clone(),
                });
                rt.accepts.push(rec);
            };
            if !e.symrel_lines.is_empty() {
                reg(
                    &mut rt,
                    "symrel",
                    ROLE_SYMREL,
                    format!("{}/escalation/symrel.r{k}.jsonl", node_dir(&n)),
                    (e.symrel_lines.join("\n") + "\n").into_bytes(),
                );
            }
            if !e.recipe_manifest.is_empty() {
                for (file, text) in &e.recipe_files {
                    rt.accepts.push(record(
                        format!("{}/escalation/r{k}/{file}", node_dir(&n)),
                        text.clone().into_bytes(),
                    ));
                }
                reg(
                    &mut rt,
                    "recipes",
                    ROLE_RECIPES,
                    format!("{}/escalation/recipes.r{k}.jsonl", node_dir(&n)),
                    (e.recipe_manifest.join("\n") + "\n").into_bytes(),
                );
            }
            if let Some(t) = &e.terminals {
                reg(
                    &mut rt,
                    "terminals",
                    ROLE_TERMINALS,
                    format!("{}/escalation/terminals.r{k}.json", node_dir(&n)),
                    json_bytes(t),
                );
            }
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
            // lane closeloop4 (audit-closeloop m8): the statement and every other phase-D file are ledger records
            if let Some(m) = f.result["final_files_blake3"].as_object() {
                for (rel, b3) in m {
                    let p = self.abs(&format!("{}/final/{rel}", node_dir(&n)));
                    let bytes = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
                    rt.events.push(Event::InputRegistered {
                        input: format!("{}:{}", input_final(&n), rel),
                        role: ROLE_FINAL_FILE.into(),
                        path: p.display().to_string(),
                        bytes,
                        blake3: b3.as_str().unwrap_or("").to_string(),
                    });
                }
            }
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
            table_blake3: file_blake3_cached(&tp)?,
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
            kind: match bf.kind.as_str() {
                "facts" => KIND_FACTS.into(),
                "exact" => KIND_EXACT.into(),
                _ => KIND_PROOF.into(),
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
        let pc = r
            .cfg
            .proof
            .as_ref()
            .ok_or("proof item without a proof config")?;
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
        } else if b.kind == "exact" {
            (
                JobKind::CoefficientFacts,
                vec![
                    file(format!("{dir}/{item}.requests.jsonl"), "requests")?,
                    table,
                ],
            )
        } else {
            (
                JobKind::ProofInstantiate,
                vec![
                    file(format!("{dir}/{item}.requests.jsonl"), "requests")?,
                    table,
                ],
            )
        };
        // lane closeloop3: a `coefficient-facts` job runs the exact-evaluation program under its own caps
        let exact = (b.kind == "exact")
            .then_some(pc.exact_eval.as_ref())
            .flatten();
        if b.kind == "exact" && exact.is_none() {
            return Err(format!(
                "{item}: an exact-eval batch without proof.exact_eval"
            ));
        }
        let fake = match exact {
            Some(x) => x
                .fake
                .as_ref()
                .map(|f| proofs::fake_script(&b, &r.exact_oracle, f.ms)),
            None => pc
                .fake
                .as_ref()
                .map(|f| proofs::fake_script(&b, &r.proof_oracle, f.ms)),
        };
        Ok(JobRequest {
            job,
            item: item.to_string(),
            rung: Rung(0),
            kind,
            priority,
            cap_bytes: exact.map(|x| x.cap_bytes).unwrap_or(pc.cap_bytes),
            expected_peak_bytes: exact
                .map(|x| x.expected_peak_bytes)
                .unwrap_or(pc.expected_peak_bytes),
            cores: if exact.is_some() {
                1
            } else {
                pc.threads as usize
            },
            heartbeat_s: exact.map(|x| x.heartbeat_s()).unwrap_or(pc.heartbeat_s()),
            stop_grace_s: 5,
            program: exact
                .map(|x| x.program_key())
                .unwrap_or(pc.program_key())
                .to_string(),
            // lane closeloop4: a second-route rr_proof job instantiates records with elided loci in facts mode
            args: if kind == JobKind::ProofInstantiate && b.reason == proofs::REASON_SECOND_ROUTE {
                vec!["--elided-loci".into(), "facts".into()]
            } else {
                vec![]
            },
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
                // lane closeloop4 (= lane t266run's version, kept once): the phase the loop is REALLY in (round 0 = loading
                // the tables; every other running pass is a check, round 1 also commits the import item; after a
                // commit: the jobs it issued, or the operator)
                (Some(p), _, _) if p.round == 0 => "loading",
                (Some(_), _, _) => "checking",
                _ if self.awaiting(st, i).is_some() => "awaiting operator",
                _ if pending > 0 => "solving",
                _ if self.proofs_pending(st, node) => "proving",
                _ => "waiting",
            };
            let lc = r.last_check.clone().unwrap_or(Value::Null);
            let answers = self.answers(st, i).len();
            // lane closeloop2: proof / fact items and their jobs
            let mut proof_by_phase: BTreeMap<String, u64> = BTreeMap::new();
            let (mut proof_running, mut proof_cpu_ms) = (0u64, 0u64);
            for it in st
                .items
                .values()
                .filter(|it| it.node == *node && (is_proof_kind(&it.kind)))
            {
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
                "pass_round": r.running.as_ref().map(|p| p.round),
                "keys": {"closure": lc["keys"], "certified": lc["certified"], "open": lc["open"],
                         "requested": requested, "pending": pending},
                "solve_items_by_phase": by_phase, "answers_files": answers,
                "jobs": {"running": running}, "cpu_s": cpu_ms as f64 / 1000.0,
                "per_key_cpu_s": per_key_s, "eta_s": eta_s,
                "pass_running_s": r.pass_started.filter(|_| r.running.is_some()).map(|t| t.elapsed().as_secs()),
                // lane t266run: which pass runs and since when (status readers see the real phase of the loop)
                "pass": r.running.as_ref().map(|p| json!({"round": p.round,
                    "kind": if p.round == 0 { "load" } else { "check" },
                    "running_s": r.pass_started.map(|t| t.elapsed().as_secs())})),
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
    /// Route one settled solve attempt (the batch's per-key decisions; closeloop.md 6.4 / 13).
    fn route_solve(
        &mut self,
        st: &CampaignState,
        i: usize,
        done: &Settled,
    ) -> Result<Routing, String> {
        let b = self.batch(st, i, &done.item)?;
        let c = &done.classification;
        let decided_row = matches!((c.row, c.sub.as_str()), (1, "") | (28, "") | (29, ""));
        let mut rt = Routing::default();
        if !(decided_row && !done.canonical_lines.is_empty() && done.table.is_some()) {
            // no per-key decision: rerun (ready) until max_job_attempts, then attention
            let what = format!("{} keys of recipe {} stay open", b.keys.len(), b.source);
            self.fail_or_attention(
                st,
                i,
                done,
                failure_code(done, "solve_job_failed"),
                &what,
                &mut rt,
            );
            return Ok(rt);
        }
        let table = done.table.clone().expect("checked");
        let set = self.ensure_set(st, i)?;
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
            max_trace_rows: r.cfg.solve.max_trace_rows,
            key_budget_s: r.cfg.solve.key_budget_s,
        };
        let d = decide(&ctx, &done.canonical_lines)?;
        if let (None, Some(e)) = (self.nodes[i].pins.get(&b.source), &d.engine_digest)
            && d.refused.is_none()
        {
            self.nodes[i].pins.insert(b.source.clone(), e.clone());
        }
        let n = self.nodes[i].n.clone();
        let dir = format!("{}/answers", node_dir(&n));
        // lane closeloop3: ERROR / IRREGULAR keys are recorded apart (retried, then attention), never as answers
        if !d.retry.is_empty() {
            let mut rb = d
                .retry
                .iter()
                .map(|(_, l)| l.as_str())
                .collect::<Vec<_>>()
                .join("\n")
                .into_bytes();
            rb.push(b'\n');
            let rr = record(format!("{dir}/{}.{}.retry.jsonl", done.item, done.job), rb);
            let rref = rr.file_ref();
            rt.events.push(Event::InputRegistered {
                input: input_retry(&done.item, &done.job),
                role: ROLE_RETRY.into(),
                path: self.abs(&rr.path).display().to_string(),
                bytes: rref.bytes,
                blake3: rref.blake3,
            });
            rt.accepts.push(rr);
        }
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
                        None if !d.retry.is_empty() => format!(
                            "no key certified ({} failures; {} keys typed ERROR / IRREGULAR are retried, then attention); the other keys go to the next recipe candidate",
                            d.failed.len(),
                            d.retry.len()
                        ),
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

    /// lane t266run: an item whose settled attempt was NOT routed (its routing was refused, or an older build left
    /// it in progress with a sealed-success attempt): a TYPED end in the ledger, never in progress forever. The
    /// attempt's result is never used: while the item has fewer than `max_job_attempts` attempts it goes back to
    /// `open` (requeue) and `ready` issues a NEW attempt, whose result goes through the normal acceptance (the
    /// supervisor's verified-stream handle); then attention `routing_refused`. A set whose effective spec does not
    /// verify goes to attention `effective_spec_mismatch` at once (a rerun cannot change it).
    fn unrouted(&self, st: &CampaignState, i: usize, item: &str, why: &str) -> Routing {
        let n_att = st
            .items
            .get(item)
            .map(|it| it.jobs.len() as u32)
            .unwrap_or(0);
        let max = self.nodes[i].cfg.budgets.max_job_attempts;
        let ev = if why.starts_with(EFFECTIVE_SPEC_MISMATCH) {
            ItemEvent::Attention {
                code: EFFECTIVE_SPEC_MISMATCH.into(),
                text: why.to_string(),
            }
        } else if n_att < max {
            ItemEvent::Requeue {
                reason: format!(
                    "{ROUTING_REFUSED}: {why}; attempt {n_att} of {max}: a new attempt (its result through the normal acceptance)"
                ),
            }
        } else {
            ItemEvent::Attention {
                code: ROUTING_REFUSED.into(),
                text: format!("{why}; {n_att} attempt(s), max_job_attempts {max}"),
            }
        };
        let mut rt = Routing::default();
        rt.events.push(Event::Item {
            item: item.to_string(),
            ev,
        });
        rt
    }

    /// lane t266run: the node's items left IN PROGRESS whose last attempt settled as a sealed success without any
    /// routing (see [`Self::unrouted`]).
    fn unrouted_items(&self, st: &CampaignState, i: usize) -> Vec<String> {
        let node = &self.nodes[i].cfg.node;
        st.items
            .iter()
            .filter(|(_, it)| {
                it.node == *node
                    && (it.kind == KIND_SOLVE || is_proof_kind(&it.kind))
                    && it.phase == ItemPhase::InProgress
                    && it
                        .jobs
                        .last()
                        .and_then(|j| st.jobs.get(j))
                        .is_some_and(|r| matches!(r.state, AttemptState::SucceededSealed(_)))
            })
            .map(|(k, _)| k.clone())
            .collect()
    }

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
        if !(c.row == 1
            && c.sub.is_empty()
            && !done.canonical_lines.is_empty()
            && done.table.is_some())
        {
            let what = format!("{} {} keys stay below their level", b.pairs().len(), b.kind);
            self.fail_or_attention(
                st,
                i,
                done,
                failure_code(done, "proof_job_failed"),
                &what,
                &mut rt,
            );
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
        let (sub, input, role) = if b.reason == proofs::REASON_SECOND_ROUTE {
            // lane closeloop4: the second route's answers are kept apart (compared with the first, never merged)
            ("facts2", input_facts2(&done.item, &done.job), ROLE_FACTS2)
        } else if proofs::is_zeros_reason(&b.reason) {
            // lane zeros2: the zeros asks' answers (verdicts + proof lines), kept apart as well
            ("zeros2", input_zeros2(&done.item, &done.job), ROLE_ZEROS2)
        } else if b.kind == "facts" || b.kind == "exact" {
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
            format!(
                "{}/{sub}/{}.{}.keys.json",
                node_dir(&n),
                done.item,
                done.job
            ),
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

// lane closeloop4 (closeloop.md 19): terminal proposals and decisions
impl CloseFromMaterialSource {
    /// The recorded terminal decisions of node `i` (ledger order; each file verified by its digest).
    fn terminal_decisions(
        &self,
        st: &CampaignState,
        i: usize,
    ) -> Result<Vec<super::terminals::TerminalDecision>, String> {
        let pre = input_tdecision_prefix(&self.nodes[i].n);
        let mut ids: Vec<(u64, String)> = st
            .inputs
            .iter()
            .filter(|(id, _)| id.starts_with(&pre))
            .map(|(id, r)| (r.registered_seq, id.clone()))
            .collect();
        ids.sort();
        ids.into_iter()
            .map(|(_, id)| {
                serde_json::from_slice(&self.input_file(st, &id)?)
                    .map_err(|e| format!("terminal decision {id}: {e}"))
            })
            .collect()
    }

    /// (round, proposals) when node `i`'s latest proposals record has no decision yet.
    fn awaiting(&self, st: &CampaignState, i: usize) -> Option<(u32, u64)> {
        let n = &self.nodes[i].n;
        if st.inputs.contains_key(&input_final(n)) {
            return None;
        }
        let k = Self::rounds_done(st, n);
        let p = st.inputs.get(&input_tproposals(n, k))?;
        let pre = input_tdecision_prefix(n);
        let decided = st
            .inputs
            .iter()
            .any(|(id, r)| id.starts_with(&pre) && r.registered_seq > p.registered_seq);
        if decided {
            return None;
        }
        let count = std::fs::read(self.abs_or(&p.path))
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .and_then(|v| v["count"].as_u64())
            .unwrap_or(0);
        Some((k, count))
    }

    /// The ledger input and the file of a terminal decision.
    fn decision_record(
        &self,
        i: usize,
        d: &super::terminals::TerminalDecision,
    ) -> Result<(Event, AcceptedRecord), String> {
        let n = &self.nodes[i].n;
        let rec = record(
            format!("{}/terminals/decision.{}.json", node_dir(n), safe(&d.id)),
            json_bytes(&serde_json::to_value(d).map_err(|e| e.to_string())?),
        );
        let fr = rec.file_ref();
        Ok((
            Event::InputRegistered {
                input: input_tdecision(n, &d.id),
                role: ROLE_TDECISION.into(),
                path: self.abs(&rec.path).display().to_string(),
                bytes: fr.bytes,
                blake3: fr.blake3,
            },
            rec,
        ))
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
                            // lane closeloop3 (closeloop.md 16.5): a boxed node is BOXED from its first ledger record
                            start_target: boxed.then_some(crate::ledger::StartTarget::Boxed),
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
                    it.node == node && (it.kind == KIND_SOLVE || is_proof_kind(&it.kind))
                })
                .map(|(k, it)| {
                    (
                        k.clone(),
                        it.kind.clone(),
                        it.phase.clone(),
                        it.jobs.clone(),
                    )
                })
                .collect();
            // lane closeloop4 (d) (lane t266run's identical hold dropped at integration): no solve job before the node's
            // set is loaded (its settle accepts against that set: never a synchronous table load on the supervisor's
            // main thread); after a restart that set is the LAST pass's (re-derived + verified, loading in the pass thread)
            let set_loaded = self.nodes[i].set.is_some();
            for (item, ikind, ph, jobs) in items {
                if ikind == KIND_SOLVE && !set_loaded {
                    continue;
                }
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
        // test builds only (supervisor/failpoint.rs): `closeloop_route_refused` refuses, `closeloop_route_dropped`
        // drops (no events: the state an older build left), the routing of every FIRST solve attempt
        let fp = crate::supervisor::failpoint::Failpoints::from_env();
        let first_solve = it.kind == KIND_SOLVE && it.jobs.len() <= 1;
        if first_solve && fp.armed("closeloop_route_dropped") {
            return Ok(Routing::default());
        }
        let routed = if is_proof_kind(&it.kind) {
            self.route_proof(st, i, done)
        } else if it.kind == KIND_SOLVE {
            if first_solve && fp.armed("closeloop_route_refused") {
                Err("failpoint closeloop_route_refused".to_string())
            } else {
                self.route_solve(st, i, done)
            }
        } else {
            return Err(format!("item {} is not a solve item", done.item));
        };
        // lane t266run: a refused routing is a typed item event (requeue as a new attempt, then attention), never an
        // item left in progress with a settled attempt
        Ok(routed.unwrap_or_else(|e| {
            eprintln!(
                "closeloop: {} {} a{}: routing refused: {e}",
                done.item, done.job, done.attempt
            );
            self.unrouted(st, i, &done.item, &e)
        }))
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
        // lane t266run: no item stays in progress over a settled attempt that was never routed
        for i in 0..self.nodes.len() {
            for item in self.unrouted_items(st, i) {
                let why =
                    "settled attempt without routing (sealed success, item still in progress)";
                eprintln!("closeloop: {item}: {why}");
                rt.events.extend(self.unrouted(st, i, &item, why).events);
            }
        }
        let mut finalized: BTreeMap<usize, String> = BTreeMap::new();
        // lane layers (closeloop.md 23): register the layers a running pass froze, NOW (an interrupt never loses them)
        for i in 0..self.nodes.len() {
            let msgs: Vec<super::layers::LayerMsg> = self.nodes[i]
                .running
                .as_ref()
                .and_then(|r| r.layer_rx.as_ref())
                .map(|rx| rx.try_iter().collect())
                .unwrap_or_default();
            for m in msgs {
                let n = self.nodes[i].n.clone();
                let input = super::layers::input_layer(&n, m.idx);
                if st.inputs.contains_key(&input) {
                    continue; // frozen once: never re-registered
                }
                let rec = record(
                    format!("{}/layers/{}.record.json", node_dir(&n), m.idx),
                    m.body,
                );
                let fr = rec.file_ref();
                rt.events.push(Event::InputRegistered {
                    input,
                    role: super::layers::ROLE_LAYER.into(),
                    path: self.abs(&rec.path).display().to_string(),
                    bytes: fr.bytes,
                    blake3: fr.blake3,
                });
                rt.accepts.push(rec);
            }
        }
        for i in 0..self.nodes.len() {
            // lane repairflow (repairflow.md 6): hold the node until the repair flow wrote its extension, then the composed spec
            let r = &mut self.nodes[i];
            if super::repair::sync_inner(st, &r.n, &mut r.spec_text, &mut r.spec_rel, &mut r.set)? {
                continue;
            }
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
                    Ok(PassResult::Loaded(s)) => {
                        self.nodes[i].chart = Some(
                            s.frame
                                .mask()
                                .iter()
                                .map(|&l| if l { '1' } else { '0' })
                                .collect(),
                        );
                        self.nodes[i].set_digest = run.digest.clone();
                        self.nodes[i].set = Some(s)
                    }
                    Ok(PassResult::Checked(o)) => {
                        self.nodes[i].chart = Some(
                            o.set
                                .frame
                                .mask()
                                .iter()
                                .map(|&l| if l { '1' } else { '0' })
                                .collect(),
                        );
                        self.nodes[i].set_digest = run.digest.clone();
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

    fn blocked_on_operator(&self, st: &CampaignState) -> Option<String> {
        // lane t266run (L17): a node with nothing left to run but items that ended at their memory cap: the operator
        // resumes with a higher --mem-cap (a launch-time resource: the same campaign)
        let mut v = vec![];
        for r in &self.nodes {
            if r.running.is_some() || st.inputs.contains_key(&input_final(&r.n)) {
                continue;
            }
            let mut held = 0usize;
            let mut moving = false;
            for (id, it) in st.items.iter() {
                // int7: the proof kinds by `is_proof_kind` (incl. closeloop3's exact-eval items), as route / unrouted
                if it.node != r.cfg.node || !(it.kind == KIND_SOLVE || is_proof_kind(&it.kind)) {
                    continue;
                }
                if Self::memory_cap_held(st, id) {
                    held += 1;
                } else if !Self::item_final(&it.phase) {
                    moving = true;
                }
            }
            if held > 0 && !moving {
                v.push(format!(
                    "node {}: {held} item(s) ended at their memory cap ({}): resume with a higher `--mem-cap SIZE` (launch-time resource, same campaign)",
                    r.cfg.node,
                    crate::launch::MEMORY_CAP_EXCEEDED
                ));
            }
        }
        (!v.is_empty()).then(|| v.join("; "))
    }

    /// lane closeloop4 (closeloop.md 19): nodes whose terminal proposals wait for the operator (exit 8); lane t266run:
    /// also a FINAL result that says `awaiting_operator` (a campaign directory written by that lane's build).
    fn operator_decision(&self, st: &CampaignState) -> Option<String> {
        let mut v: Vec<String> = (0..self.nodes.len())
            .filter(|&i| self.nodes[i].running.is_none())
            .filter_map(|i| {
                self.awaiting(st, i).map(|(k, n)| {
                    format!(
                        "{}: open: awaiting operator decision on {n} terminal proposals (round {k}; `rr_campaign control CDIR resolve-terminals --node {} --accept KEYS_FILE | --reject`)",
                        self.nodes[i].cfg.node, self.nodes[i].cfg.node
                    )
                })
            })
            .collect();
        for r in &self.nodes {
            if let Ok(b) = self.input_file(st, &input_final(&r.n))
                && let Ok(x) = serde_json::from_slice::<Value>(&b)
                && let Some(m) = x["awaiting_operator"].as_str()
            {
                v.push(format!("node {}: {m}", r.cfg.node));
            }
        }
        (!v.is_empty()).then(|| v.join(" | "))
    }

    /// lane closeloop4 (closeloop.md 19): the operator's `resolve-terminals` request.
    fn resolve_operator_decision(
        &mut self,
        st: &CampaignState,
        req: &crate::supervisor::ControlRequest,
    ) -> Result<Routing, String> {
        let mut rt = Routing::default();
        let targets: Vec<usize> = (0..self.nodes.len())
            .filter(|&i| {
                req.target
                    .as_deref()
                    .is_none_or(|t| t == self.nodes[i].cfg.node)
            })
            .filter(|&i| self.awaiting(st, i).is_some())
            .collect();
        if targets.is_empty() {
            return Err(format!(
                "no node {}awaits a terminal decision",
                req.target
                    .as_deref()
                    .map(|t| format!("{t} "))
                    .unwrap_or_default()
            ));
        }
        let p = req.payload.clone().unwrap_or(Value::Null);
        let accept: Option<Vec<Vec<i32>>> = if p["reject"] == true {
            None
        } else {
            Some(
                serde_json::from_value(p["accept"].clone())
                    .map_err(|e| format!("payload accept: {e}"))?,
            )
        };
        for i in targets {
            let (k, _) = self.awaiting(st, i).expect("awaiting");
            let pr: Value = serde_json::from_slice(
                &self.input_file(st, &input_tproposals(&self.nodes[i].n, k))?,
            )
            .map_err(|e| format!("proposals: {e}"))?;
            let proposed: Vec<Vec<i32>> = pr["proposals"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|x| serde_json::from_value(x["key"].clone()).ok())
                .collect();
            // with several nodes, each takes the listed keys that are its own proposals
            let acc: Option<Vec<Vec<i32>>> = accept.as_ref().map(|a| {
                if req.target.is_some() {
                    a.clone()
                } else {
                    a.iter().filter(|k| proposed.contains(k)).cloned().collect()
                }
            });
            let d = super::terminals::operator_decision(
                &self.nodes[i].cfg.node,
                k,
                &proposed,
                acc.as_deref(),
                &req.id,
                if req.by.is_empty() {
                    "operator"
                } else {
                    &req.by
                },
                req.reason.as_deref().unwrap_or(""),
            )?;
            let (ev, rec) = self.decision_record(i, &d)?;
            rt.events.push(ev);
            rt.accepts.push(rec);
            rt.events.push(Event::Note {
                text: format!(
                    "closeloop {}: terminal decision {} by {}: accepted {}, rejected {}",
                    d.node,
                    d.id,
                    d.by,
                    d.accept.len(),
                    d.reject.len()
                ),
            });
        }
        Ok(rt)
    }

    /// lane closeloop3 (procfix5, supervisor.md 19.4): the outcome from every node's FINAL verdict (the recorded
    /// `final/result.json`): `closed` -> Closed; `error` (or an unreadable result) -> Error; anything else (`open`,
    /// `budget`, no result) -> Open. Error wins over Open.
    fn outcome(&self, st: &CampaignState) -> CampaignOutcome {
        let (mut open, mut error) = (vec![], vec![]);
        for r in &self.nodes {
            let node = &r.cfg.node;
            let Some(inp) = st.inputs.get(&input_final(&r.n)) else {
                open.push(format!("{node}: no final verdict"));
                continue;
            };
            // lane closeloop4 (audit-closeloop m8): a changed / truncated statement or phase-D file is an error
            let pre = format!("{}:", input_final(&r.n));
            let bad: Vec<String> = st
                .inputs
                .keys()
                .filter(|id| id.starts_with(&pre))
                .filter(|id| self.input_file(st, id).is_err())
                .cloned()
                .collect();
            if !bad.is_empty() {
                error.push(format!(
                    "{node}: phase-D file(s) changed since they were recorded: {}",
                    bad.join(", ")
                ));
                continue;
            }
            // lane closeloop4 (audit-closeloop F-OUT): the result is read through its RECORDED digest (input_file
            // refuses a changed file: then the verdict is unreadable -> error), never the bare file
            let v: Option<Value> = self
                .input_file(st, &input_final(&r.n))
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok());
            match v.as_ref().and_then(|v| v["verdict"].as_str()) {
                Some("closed") => {}
                Some("error") => error.push(format!(
                    "{node}: error ({})",
                    v.as_ref().and_then(|v| v["error"].as_str()).unwrap_or("")
                )),
                Some(w) => open.push(format!(
                    "{node}: {w} ({})",
                    v.as_ref()
                        .and_then(|v| v["blockers"].as_array())
                        .map(|b| b
                            .iter()
                            .filter_map(Value::as_str)
                            .take(5)
                            .collect::<Vec<_>>()
                            .join("; "))
                        .unwrap_or_default()
                )),
                None => error.push(format!("{node}: final result unreadable ({})", inp.path)),
            }
        }
        if !error.is_empty() {
            CampaignOutcome::Error(error.join(" | "))
        } else if !open.is_empty() {
            CampaignOutcome::Open(open.join(" | "))
        } else {
            CampaignOutcome::Closed
        }
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
