//! The REPAIR FLOW phase of the closing loop (interfaces/repairflow.md section 1): cheap symbolic first, else recipe
//! export, per uncovered face of a node's closure.
//!
//! `RepairFlowSource` is a DECORATOR around any closing-loop work source (the closeloop wrapped by termmin). For a node
//! with the flow enabled (campaign input of role `repairflow`, schema `rustred.campaign.repairflow.v1`):
//! - bootstrap registers the node's effective POLICY (input `repairflow:<n>:policy`); from then on the closing loop
//!   HOLDS the node (hook [`sync_inner`] in the closeloop's poll) until the EXTENSION exists;
//! - P1 (own pass thread): the node's effective import spec is loaded and checked; the uncovered open keys are grouped
//!   into faces; the shadowing test runs per face; the check record is input `repairflow:<n>:check`; one item of kind
//!   `repair-symbolic` per searched face;
//! - P2 / P3: one generate job per face (cheap budget, pre-screen with the size caps, children returned not searched);
//!   a settled attempt -> answer file (`repairflow:<item>:answer:<job>`) with the candidate rule lines (raw lines bound
//!   to the verified canonical table), their measures and the open keys they serve; item `Produce` / `Escalate`;
//! - P4 (barrier: every S item settled): the core decision (`rustred_campaign_core::repairflow::decide`), recorded as
//!   `repairflow:<n>:decisions` (recomputed on resume and compared), the kept rules re-homed into one table in the
//!   node's frame, the face recipes, the COMPOSED import spec (the closeloop's spec + the extension sources before the
//!   default recipe) = input `repairflow:<n>:extension`; items `Accept` (geometry: the closing loop certifies the keys)
//!   or `Reject` (class);
//! - P5 (after the closing loop's final result): the report and the export directory.
//!
//! Generic (L8): nothing here knows a node, an arity or a count.

use super::config::{CONFIG_ROLE, CloseLoopConfig, NodeCfg};
use super::ids::{input_final, input_import_spec, node_dir, safe};
use crate::ledger::{AcceptSource, Event, ItemEvent};
use crate::state::{
    AttemptState, CampaignState, EvidenceLevel, ItemPhase, NodeEvent, NodePhase, Rung,
};
use crate::supervisor::spec::CampaignSpec;
use crate::supervisor::worksource::{
    AcceptedRecord, JobRequest, Routing, Settled, SourceContext, WorkSource,
};
use rustred_campaign_contract::classify::OutcomeKind;
use rustred_campaign_contract::fake::{FakeOp, FakeRule, FakeScript, OnSigterm, RssPlan};
use rustred_campaign_contract::spec::{
    Budgets as JobBudgets, InputRef, JobKind, WallBudgets, WorkBudgets,
};
use rustred_campaign_contract::stop::{CaseResult, StopCode};
use rustred_campaign_core::import::check::{CheckOptions, RowStatus};
use rustred_campaign_core::import::{Evidence, ImportedSet, TableSetSpec};
use rustred_campaign_core::key::NodeFrame;
use rustred_campaign_core::partition::DomainSource;
use rustred_campaign_core::repairflow::{
    self as rf, CandidateAnswer, FaceAnswer, FaceGroup, NodeDecisions, PolicyFile, RepairPolicy,
    ShadowClass,
};
use rustred_campaign_core::worklist::items::Face;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread::JoinHandle;

/// Campaign input role of the policy file.
pub const ROLE: &str = "repairflow";
pub const ROLE_POLICY: &str = "repairflow_policy";
pub const ROLE_CHECK: &str = "repairflow_check";
pub const ROLE_ANSWER: &str = "repairflow_answer";
pub const ROLE_DECISIONS: &str = "repairflow_decisions";
pub const ROLE_EXTENSION: &str = "repairflow_extension";
pub const ROLE_RESULT: &str = "repairflow_result";
pub const KIND_SYMBOLIC: &str = "repair-symbolic";
pub const CHECK_SCHEMA: &str = "rustred.campaign.repairflow-check.v1";
pub const ANSWER_SCHEMA: &str = "rustred.campaign.repairflow-answer.v1";
pub const REPORT_SCHEMA: &str = "rustred.campaign.repairflow-report.v1";

pub fn input_policy(n: &str) -> String {
    format!("repairflow:{n}:policy")
}
pub fn input_check(n: &str) -> String {
    format!("repairflow:{n}:check")
}
pub fn input_decisions(n: &str) -> String {
    format!("repairflow:{n}:decisions")
}
pub fn input_extension(n: &str) -> String {
    format!("repairflow:{n}:extension")
}
pub fn input_report(n: &str) -> String {
    format!("repairflow:{n}:report")
}
pub fn input_withheld(n: &str) -> String {
    format!("repairflow:{n}:withheld")
}
pub const ROLE_WITHHELD: &str = "repairflow_withheld";
pub const MANIFEST_SCHEMA: &str = "rustred.campaign.recipe-export.v1";
pub fn input_answer(item: &str, job: &str) -> String {
    format!("repairflow:{item}:answer:{job}")
}
pub fn s_item(n: &str, f: usize) -> String {
    format!("{n}.rf.s{f:05}")
}
fn rf_dir(n: &str) -> String {
    format!("{}/repairflow", node_dir(n))
}

fn blake3_hex(b: &[u8]) -> String {
    blake3::hash(b).to_hex().to_string()
}

fn json_bytes(v: &Value) -> Vec<u8> {
    let mut b = serde_json::to_vec_pretty(v).expect("json");
    b.push(b'\n');
    b
}

fn sha256_hex(b: &[u8]) -> String {
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(b);
    h.finish_hex()
}

/// Read a registered input, verified by its recorded BLAKE3.
fn read_input(st: &CampaignState, id: &str) -> Result<Vec<u8>, String> {
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

// ------------------------------------------------------------------------------------------------------------------
// hooks for the closing loop and termmin (interfaces/repairflow.md section 6)

/// HOOK of the closeloop's poll: `Ok(true)` = HOLD the node (the flow is enabled and its extension is not written yet:
/// no pass this tick); `Ok(false)` = go on, with the COMPOSED spec in force once the extension exists (the cached set
/// is dropped when the spec changes). A changed extension file is a refusal.
pub fn sync_inner(
    st: &CampaignState,
    n: &str,
    spec_text: &mut String,
    spec_rel: &mut String,
    set: &mut Option<Arc<ImportedSet>>,
) -> Result<bool, String> {
    if !st.inputs.contains_key(&input_policy(n)) {
        return Ok(false);
    }
    let Some(r) = st.inputs.get(&input_extension(n)) else {
        return Ok(true);
    };
    let rel = format!("{}/import-spec.composed.json", rf_dir(n));
    if *spec_rel == rel {
        return Ok(false);
    }
    let b = read_input(st, &input_extension(n))?;
    let text = String::from_utf8(b).map_err(|e| format!("composed spec: {e}"))?;
    let _ = r;
    *spec_text = text;
    *spec_rel = rel;
    *set = None;
    Ok(false)
}

/// HOOK of termmin's phase E load: the node's effective import spec text (the composed one when the flow wrote it).
pub fn effective_spec_text(st: &CampaignState, n: &str) -> Result<String, String> {
    let id = if st.inputs.contains_key(&input_extension(n)) {
        input_extension(n)
    } else {
        input_import_spec(n)
    };
    String::from_utf8(read_input(st, &id)?).map_err(|e| format!("effective import spec: {e}"))
}

// ------------------------------------------------------------------------------------------------------------------
// the decorator

/// Test-only fake answers of the generate jobs (`fake` section of the policy file): face label -> behaviour.
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FakeGen {
    #[serde(default = "d_fake_program")]
    program: String,
    #[serde(default)]
    ms: u64,
    /// face label -> {"rule": {record}} | {"stop": "budget_wall" | "open_obligations" | ...}
    faces: BTreeMap<String, Value>,
}

fn d_fake_program() -> String {
    "rr_fake_child".into()
}

/// The P1 result of a node.
struct P1 {
    faces: Vec<FaceGroup>,
    shadow: Vec<ShadowClass>,
    outside: Vec<Vec<i32>>,
    frame: NodeFrame,
    frame_header: Value,
    frame_header_line: String,
    line_prio: Vec<usize>,
    num_prio: Vec<usize>,
    /// The check record (JSON bytes recorded as input).
    record: Vec<u8>,
    /// The generate jobs' family text, derived from the frame header (None: the header carries none; the closeloop's
    /// solve family is used and the pre-flight job decides).
    family: Option<(String, &'static str)>,
    /// ROUTE 0b: the point-record table of the kept symmetry relations (None: none kept / route off).
    r0b_table: Option<String>,
}

struct RfNode {
    cfg: NodeCfg,
    n: String,
    policy: RepairPolicy,
    pass: Option<JoinHandle<Result<P1, String>>>,
    p1: Option<Arc<P1>>,
    fake: Option<FakeGen>,
}

pub struct RepairFlowSource<S: WorkSource> {
    inner: S,
    cdir: PathBuf,
    nodes: Vec<RfNode>,
    programs: BTreeMap<String, PathBuf>,
}

/// Wrap the closing loop's source (transparent when the campaign has no input of role `repairflow`).
pub fn wrap<S: WorkSource>(
    inner: S,
    spec: &CampaignSpec,
    cdir: &Path,
) -> Result<RepairFlowSource<S>, String> {
    let mut nodes = vec![];
    let pin: Vec<_> = spec.inputs.iter().filter(|i| i.role == ROLE).collect();
    if pin.len() > 1 {
        return Err(format!("repairflow: at most one input of role {ROLE}"));
    }
    if let Some(p) = pin.first() {
        let text = std::fs::read_to_string(&p.path)
            .map_err(|e| format!("repairflow policy {}: {e}", p.path.display()))?;
        let raw: Value =
            serde_json::from_str(&text).map_err(|e| format!("repairflow policy: {e}"))?;
        let fake: Option<FakeGen> = match raw.get("fake") {
            Some(f) => Some(
                serde_json::from_value(f.clone()).map_err(|e| format!("repairflow fake: {e}"))?,
            ),
            None => None,
        };
        let mut pv = raw.clone();
        if let Some(o) = pv.as_object_mut() {
            o.remove("fake");
        }
        let file = PolicyFile::parse(&pv.to_string())?;
        let cin = spec
            .inputs
            .iter()
            .find(|i| i.role == CONFIG_ROLE)
            .ok_or("repairflow: no closeloop config input")?;
        let cfg = CloseLoopConfig::read(&cin.path)?;
        for nc in &cfg.nodes {
            let policy = file.effective(Some(&nc.node))?;
            if !policy.enabled {
                continue;
            }
            nodes.push(RfNode {
                cfg: nc.clone(),
                n: safe(&nc.node),
                policy,
                pass: None,
                p1: None,
                fake: fake.clone(),
            });
        }
    }
    Ok(RepairFlowSource {
        inner,
        cdir: cdir.to_path_buf(),
        nodes,
        programs: spec
            .programs
            .iter()
            .map(|(k, p)| (k.clone(), p.path.clone()))
            .collect(),
    })
}

fn record(path: String, bytes: Vec<u8>) -> AcceptedRecord {
    AcceptedRecord { path, bytes }
}

impl<S: WorkSource> RepairFlowSource<S> {
    fn abs(&self, rel: &str) -> PathBuf {
        self.cdir.join(rel)
    }

    fn reg(&self, rt: &mut Routing, id: String, role: &str, rec: AcceptedRecord) {
        let f = rec.file_ref();
        rt.events.push(Event::InputRegistered {
            input: id,
            role: role.into(),
            path: self.abs(&rec.path).display().to_string(),
            bytes: f.bytes,
            blake3: f.blake3,
        });
        rt.accepts.push(rec);
    }

    fn node_of_item(&self, st: &CampaignState, item: &str) -> Option<usize> {
        let it = st.items.get(item)?;
        if it.kind != KIND_SYMBOLIC {
            return None;
        }
        self.nodes.iter().position(|r| r.cfg.node == it.node)
    }

    fn s_items(&self, st: &CampaignState, i: usize) -> Vec<(String, ItemPhase, Vec<String>)> {
        let node = &self.nodes[i].cfg.node;
        st.items
            .iter()
            .filter(|(_, it)| it.node == *node && it.kind == KIND_SYMBOLIC)
            .map(|(k, it)| (k.clone(), it.phase.clone(), it.jobs.clone()))
            .collect()
    }

    /// Start the P1 pass of node `i` (thread).
    fn start_p1(&mut self, st: &CampaignState, i: usize) -> Result<(), String> {
        let r = &self.nodes[i];
        let text = String::from_utf8(read_input(st, &input_import_spec(&r.n))?)
            .map_err(|e| format!("import spec: {e}"))?;
        let base = r.cfg.base();
        let policy = r.policy.clone();
        let node = r.cfg.node.clone();
        let threads = r.cfg.check_threads.max(1);
        let rows = self.abs(&format!("{}/check.rows.jsonl", rf_dir(&r.n)));
        let h = std::thread::Builder::new()
            .name(format!("repairflow-p1-{}", r.n))
            .spawn(move || p1_pass(&node, &text, &base, &policy, threads, &rows))
            .map_err(|e| format!("p1 thread: {e}"))?;
        self.nodes[i].pass = Some(h);
        Ok(())
    }

    fn job_request(
        &self,
        i: usize,
        item: &str,
        job: String,
        face: &FaceGroup,
        prio: i64,
    ) -> Result<JobRequest, String> {
        let r = &self.nodes[i];
        let p1 = r.p1.as_ref().ok_or("p1 not loaded")?;
        let sp = &r.policy.symbolic;
        let cases_rel = format!("{}/cases/{item}.jsonl", rf_dir(&r.n));
        let cases_abs = self.abs(&cases_rel);
        let line = match &face.exception {
            Some(x) => x.case.to_string(),
            None => rf::case_line(&face.face(), p1.frame.mask()).to_string(),
        } + "\n";
        if std::fs::read(&cases_abs).ok().as_deref() != Some(line.as_bytes()) {
            if let Some(d) = cases_abs.parent() {
                std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
            }
            crate::atomic::write_atomic(&cases_abs, line.as_bytes())
                .map_err(|e| format!("{}: {e}", cases_abs.display()))?;
        }
        // the family input DERIVED FROM THE TABLE SET (frame header), written once (content-addressed by its digest)
        let (fam, fam_b3) = match &p1.family {
            Some((text, _)) => {
                let b3 = blake3_hex(text.as_bytes());
                let p = self.abs(&format!("{}/family.{}.sym", rf_dir(&r.n), &b3[..16]));
                if std::fs::read(&p).ok().as_deref() != Some(text.as_bytes()) {
                    crate::atomic::write_atomic(&p, text.as_bytes())
                        .map_err(|e| format!("{}: {e}", p.display()))?;
                }
                (p, b3)
            }
            None => {
                let f = r.cfg.solve.family.clone();
                let b3 = crate::atomic::blake3_file(&f)
                    .map_err(|e| format!("{}: {e}", f.display()))?
                    .1;
                (f, b3)
            }
        };
        let fam = &fam;
        let args = rf::s_args(
            &face.face(),
            &r.policy,
            &r.cfg.solve.order_arg(),
            &p1.line_prio,
            &p1.num_prio,
        );
        let (program, fake) = match &r.fake {
            Some(f) => (f.program.clone(), Some(fake_gen_script(f, &face.label))),
            None => (r.cfg.solve.program.clone(), None),
        };
        if !self.programs.contains_key(&program) {
            return Err(format!("program {program} not in the spec"));
        }
        Ok(JobRequest {
            job,
            item: item.into(),
            rung: Rung(0),
            kind: JobKind::Generate,
            priority: prio,
            cap_bytes: sp.cap_bytes,
            expected_peak_bytes: sp.expected_peak_bytes,
            cores: sp.threads.max(1) as usize,
            heartbeat_s: sp.heartbeat_s,
            stop_grace_s: sp.stop_grace_s,
            program,
            args,
            inputs: vec![
                InputRef {
                    role: "family".into(),
                    path: fam.display().to_string(),
                    blake3: fam_b3,
                    upto: None,
                },
                InputRef {
                    role: "cases".into(),
                    path: cases_abs.display().to_string(),
                    blake3: blake3_hex(line.as_bytes()),
                    upto: None,
                },
            ],
            depends_on: vec![input_check(&r.n), input_policy(&r.n)],
            budgets: JobBudgets {
                work: WorkBudgets {
                    cases: Some(1),
                    ..WorkBudgets::default()
                },
                wall: WallBudgets {
                    case_s: Some(sp.wall_case_s),
                    job_s: None,
                },
                mem_soft_bytes: None,
            },
            sample_seed: 0,
            fake,
        })
    }

    /// The face of an S item (its label in the ledger).
    fn face_of(&self, st: &CampaignState, i: usize, item: &str) -> Option<FaceGroup> {
        let label = &st.items.get(item)?.label;
        self.nodes[i]
            .p1
            .as_ref()?
            .faces
            .iter()
            .find(|f| &f.label == label)
            .cloned()
    }

    /// P4: every S item settled -> decide, write the extension.
    fn barrier(&mut self, st: &CampaignState, i: usize, rt: &mut Routing) -> Result<(), String> {
        let n = self.nodes[i].n.clone();
        let node = self.nodes[i].cfg.node.clone();
        let p1 = self.nodes[i].p1.clone().ok_or("p1 not loaded")?;
        let items = self.s_items(st, i);
        // an item in attention (its job was refused by the contract: a bug, not evidence about the face) holds the
        // barrier until the operator retries or skips it (a skipped face goes to its recipes)
        if items.iter().any(|(_, ph, _)| {
            matches!(
                ph,
                ItemPhase::Open | ItemPhase::InProgress | ItemPhase::Attention(_)
            )
        }) {
            return Ok(());
        }
        // answers by face label (the last answer input of each item)
        let mut answers: BTreeMap<String, FaceAnswer> = BTreeMap::new();
        let mut produced: Vec<(String, String)> = vec![]; // (item, face label)
        for (item, ph, jobs) in &items {
            let label = st
                .items
                .get(item)
                .map(|x| x.label.clone())
                .unwrap_or_default();
            for job in jobs.iter().rev() {
                let id = input_answer(item, job);
                if st.inputs.contains_key(&id) {
                    let v: Value = serde_json::from_slice(&read_input(st, &id)?)
                        .map_err(|e| format!("answer {id}: {e}"))?;
                    let a: FaceAnswer = serde_json::from_value(v["answer"].clone())
                        .map_err(|e| format!("answer {id}: {e}"))?;
                    answers.insert(label.clone(), a);
                    break;
                }
            }
            if *ph == ItemPhase::Produced {
                produced.push((item.clone(), label));
            }
        }
        let d = rf::decide(
            &node,
            &p1.faces,
            &p1.shadow,
            &answers,
            &p1.outside,
            &self.nodes[i].policy,
        );
        let mut d = d;
        for (fd, fg) in d.faces.iter_mut().zip(&p1.faces) {
            fd.layers = rf::faces::layers(fg, p1.frame.mask());
        }
        let dbytes = json_bytes(&serde_json::to_value(&d).map_err(|e| e.to_string())?);
        if let Some(r) = st.inputs.get(&input_decisions(&n))
            && r.blake3 != blake3_hex(&dbytes)
        {
            return Err(format!(
                "node {node}: the recomputed repair decisions differ from the recorded ones: refused (U-1)"
            ));
        }
        let dir = rf_dir(&n);
        // the re-homed symbolic table (frame header + kept lines, index renumbered, `type` first)
        let mut kept_lines: Vec<String> = vec![];
        for f in &d.faces {
            if let Some(sha) = &f.kept
                && let Some(FaceAnswer::Candidates { candidates, .. }) = answers.get(&f.face)
                && let Some(c) = candidates.iter().find(|c| &c.record_sha256 == sha)
            {
                kept_lines.push(rehome_line(&c.line, kept_lines.len())?);
            }
        }
        let mut table_rel = None;
        if !kept_lines.is_empty() {
            let mut t = p1.frame_header_line.clone();
            t.push('\n');
            for l in &kept_lines {
                t.push_str(l);
                t.push('\n');
            }
            let rel = format!("{dir}/symbolic.table.jsonl");
            rt.accepts.push(record(rel.clone(), t.into_bytes()));
            table_rel = Some(rel);
        }
        // face recipes
        let ctx_order = self.nodes[i].cfg.solve.order_arg();
        let pd = self.nodes[i].policy.digest();
        let rctx = rf::RecipeContext {
            node: &node,
            node_order: &ctx_order,
            policy_digest: &pd,
            mixed_shells: self.nodes[i].policy.recipes.mixed_shells.clone(),
        };
        let mut files: BTreeMap<(String, String), String> = BTreeMap::new();
        let mut recipe_errors: Vec<Value> = vec![];
        for f in &d.faces {
            let face = Face::new(f.sd.clone(), f.sn.clone());
            for s in &f.recipe_steps {
                match rf::face_recipe(
                    &face,
                    s,
                    p1.frame.mask(),
                    &p1.line_prio,
                    &p1.num_prio,
                    &rctx,
                ) {
                    Ok(r) => {
                        let rel = format!("{dir}/recipes/{}.{s}.json", rf::face_tag(&face));
                        let mut b = serde_json::to_vec(&r).map_err(|e| e.to_string())?;
                        b.push(b'\n');
                        rt.accepts.push(record(rel.clone(), b));
                        files.insert(
                            (f.face.clone(), s.clone()),
                            self.abs(&rel).display().to_string(),
                        );
                    }
                    Err(e) => recipe_errors.push(json!({"face": f.face, "step": s, "error": e})),
                }
            }
        }
        let mut ext0: Vec<Value> = vec![];
        if let Some(t) = &p1.r0b_table {
            let rel = format!("{dir}/symrel.table.jsonl");
            // (re)written: the P1 commit wrote the same bytes; a resume recomputes them identically
            rt.accepts.push(record(rel.clone(), t.clone().into_bytes()));
            let mut src = json!({"id": "rf:symmetry-relations", "kind": "supplied_relations",
                "class": "symmetry-relation", "path": self.abs(&rel).display().to_string(),
                "sha256": sha256_hex(t.as_bytes())});
            src["verify"] = match &self.nodes[i].policy.route0.family {
                Some(f) => json!({"family": {"path": f}}),
                None => json!({}),
            };
            ext0.push(src);
        }
        let ext = rf::extension_sources(
            &d,
            table_rel
                .as_ref()
                .map(|r| self.abs(r).display().to_string())
                .as_deref(),
            &files,
        );
        // the composed spec: the closeloop's effective spec + the extension BEFORE the default recipe
        let base_text = String::from_utf8(read_input(st, &input_import_spec(&n))?)
            .map_err(|e| format!("import spec: {e}"))?;
        let (base_text, _) = with_route0(&base_text, &self.nodes[i].policy)?;
        let mut v: Value = serde_json::from_str(&base_text).map_err(|e| e.to_string())?;
        let srcs = v
            .get_mut("sources")
            .and_then(Value::as_array_mut)
            .ok_or("import spec: no sources")?;
        let at = srcs
            .iter()
            .position(|s| s["id"] == "default-recipe")
            .unwrap_or(srcs.len());
        for (k, s) in ext0.into_iter().chain(ext).enumerate() {
            srcs.insert(at + k, s);
        }
        // (the import spec refuses unknown fields: the policy / decisions digests live in the decisions and report)
        let composed = serde_json::to_string_pretty(&v).expect("json") + "\n";
        if !st.inputs.contains_key(&input_decisions(&n)) {
            let rec = record(format!("{dir}/decisions.json"), dbytes.clone());
            self.reg(rt, input_decisions(&n), ROLE_DECISIONS, rec);
        }
        if !recipe_errors.is_empty() {
            rt.accepts.push(record(
                format!("{dir}/recipe-errors.json"),
                json_bytes(&json!(recipe_errors)),
            ));
        }
        // items: produced -> Accept (kept) / Reject (class)
        for (item, label) in produced {
            let fd = d.faces.iter().find(|f| f.face == label);
            let job = st
                .items
                .get(&item)
                .and_then(|x| x.jobs.last().cloned())
                .unwrap_or_default();
            let ans_ref = st.inputs.get(&input_answer(&item, &job));
            let produced_by = st.items.get(&item).and_then(|x| x.produced.clone());
            match (fd, ans_ref, produced_by) {
                (Some(f), Some(a), Some(pb)) if f.kept.is_some() => {
                    rt.events.push(Event::Item {
                        item: item.clone(),
                        ev: ItemEvent::Accept {
                            rule: format!("{item}:{}", &f.kept.clone().unwrap_or_default()[..16]),
                            level: EvidenceLevel::GeometryAccepted,
                            record: crate::ledger::FileRef {
                                path: a.path.clone(),
                                bytes: a.bytes,
                                blake3: a.blake3.clone(),
                            },
                            // bound to the sealed table that produced the item (ledger rule)
                            source: AcceptSource::Job {
                                job: pb.job.clone(),
                                attempt: pb.attempt,
                                table_blake3: pb.table.blake3.clone(),
                            },
                            elided: false,
                            evidence: None,
                            round: Some(0),
                        },
                    });
                }
                (fd, _, _) => {
                    // rejected (the class recorded), then superseded: the face goes to its recipe escalation
                    let reason = fd
                        .map(|f| format!("repairflow: {}", class_text(&f.symbolic.class)))
                        .unwrap_or_else(|| "repairflow: face not in the decisions".into());
                    rt.events.push(Event::Item {
                        item: item.clone(),
                        ev: ItemEvent::Reject {
                            reason,
                            disproof: None,
                        },
                    });
                    rt.events.push(Event::Item {
                        item: item.clone(),
                        ev: ItemEvent::Escalate {
                            to: Rung(1),
                            reason: "repairflow: the face goes to its recipe escalation".into(),
                        },
                    });
                }
            }
        }
        let rec = record(
            format!("{dir}/import-spec.composed.json"),
            composed.into_bytes(),
        );
        self.reg(rt, input_extension(&n), ROLE_EXTENSION, rec);
        Ok(())
    }

    /// WITHHOLD the closing loop's open declaration (AttemptEnded + DeclareOpen) of a flow node whose final verdict is
    /// `open`: P5 decides whether the terminal proposals close it (else the events are released unchanged).
    fn filter_inner(&mut self, mut rt: Routing) -> Routing {
        for r in &self.nodes {
            if !r.policy.proposals.enabled || !r.policy.proposals.declare_terminal {
                continue;
            }
            let fin = input_final(&r.n);
            let Some(fr) = rt.events.iter().find_map(|e| match e {
                Event::InputRegistered { input, path, .. } if *input == fin => Some(path.clone()),
                _ => None,
            }) else {
                continue;
            };
            let res: Value = rt
                .accepts
                .iter()
                .find(|a| self.abs(&a.path).display().to_string() == fr || a.path == fr)
                .and_then(|a| serde_json::from_slice(&a.bytes).ok())
                .unwrap_or(Value::Null);
            if res["verdict"] != "open" {
                continue;
            }
            let node = r.cfg.node.clone();
            let mut withheld = vec![];
            rt.events.retain(|e| match e {
                Event::Node {
                    node: x,
                    ev: NodeEvent::AttemptEnded { .. } | NodeEvent::DeclareOpen { .. },
                } if *x == node => {
                    withheld.push(e.clone());
                    false
                }
                _ => true,
            });
            let w = record(
                format!("{}/withheld.json", rf_dir(&r.n)),
                json_bytes(
                    &json!({"schema": "rustred.campaign.repairflow-withheld.v1", "node": node,
                    "events": withheld}),
                ),
            );
            let f = w.file_ref();
            rt.events.push(Event::InputRegistered {
                input: input_withheld(&r.n),
                role: ROLE_WITHHELD.into(),
                path: self.abs(&w.path).display().to_string(),
                bytes: f.bytes,
                blake3: f.blake3,
            });
            rt.accepts.push(w);
        }
        rt
    }

    fn closeout_check(&self, i: usize, fin: &Value, open: usize) -> (bool, String) {
        closeout_check(&self.nodes[i].policy.proposals, fin, open)
    }

    /// Per recipe source: solve jobs, keys and the measured CPU seconds per key (from the closing loop's batches).
    fn solve_costs(&self, st: &CampaignState, i: usize) -> BTreeMap<String, (u64, u64, Vec<f64>)> {
        let node = &self.nodes[i].cfg.node;
        let mut m: BTreeMap<String, (u64, u64, Vec<f64>)> = BTreeMap::new();
        for (item, it) in st
            .items
            .iter()
            .filter(|(_, it)| it.node == *node && it.kind == super::ids::KIND_SOLVE)
        {
            let Ok(b) = read_input(st, &super::ids::input_batch(item)) else {
                continue;
            };
            let Ok(bf) = serde_json::from_slice::<super::accept::BatchFile>(&b) else {
                continue;
            };
            for j in &it.jobs {
                let Some(rd) = st.jobs.get(j).and_then(|x| x.run_dir.clone()) else {
                    continue;
                };
                let c = super::termmin::cost_of(Path::new(&rd), j);
                let e = m.entry(bf.source.clone()).or_default();
                e.0 += 1;
                e.1 += bf.keys.len() as u64;
                if let Some(cpu) = c.cpu_s {
                    e.2.push(cpu / bf.keys.len().max(1) as f64);
                }
            }
        }
        m
    }

    /// P5: the report after the closing loop's final result: per face the decision, keys by class, terminal
    /// proposals (and the close-out by them), the SELF-CONTAINED export (recipes in escalation order, certified key
    /// sets with per-key evidence, the default sequence, measured per-key solve costs, run-time budgets, digests).
    fn report(&mut self, st: &CampaignState, i: usize, rt: &mut Routing) -> Result<(), String> {
        let n = self.nodes[i].n.clone();
        let node = self.nodes[i].cfg.node.clone();
        let dir = rf_dir(&n);
        let d: NodeDecisions = serde_json::from_slice(&read_input(st, &input_decisions(&n))?)
            .map_err(|e| format!("decisions: {e}"))?;
        let fin: Value =
            serde_json::from_slice(&read_input(st, &input_final(&n))?).unwrap_or(Value::Null);
        // the final statement rows: per key status + server source + evidence
        let stmt = self.abs(&format!("{}/final/statement/statement.jsonl", node_dir(&n)));
        let mut by_server: BTreeMap<String, Vec<Value>> = BTreeMap::new();
        let mut by_class: BTreeMap<String, u64> = BTreeMap::new();
        let mut open_keys: Vec<Value> = vec![];
        let mut stmt_lines: Vec<String> = vec![];
        // per plain face of the key: (served by tables, served by recipes, open)
        let mask: Vec<bool> = self.nodes[i]
            .p1
            .as_ref()
            .map(|p| p.frame.mask().to_vec())
            .unwrap_or_default();
        let mut per_face: BTreeMap<String, (u64, u64, u64)> = BTreeMap::new();
        if let Ok(t) = std::fs::read_to_string(&stmt) {
            for l in t.lines().filter(|l| !l.trim().is_empty()) {
                stmt_lines.push(l.to_string());
                let Ok(r) = serde_json::from_str::<Value>(l) else {
                    continue;
                };
                let status = r["status"].as_str().unwrap_or("");
                let src = r["server"]["source"].as_str().unwrap_or("").to_string();
                let class = r["server"]["class"].as_str().unwrap_or("").to_string();
                let fl = serde_json::from_value::<Vec<i32>>(r["key"].clone())
                    .ok()
                    .and_then(|k| Face::of_key(&k, &mask))
                    .map(|f| f.label());
                if let Some(fl) = fl {
                    let e = per_face.entry(fl).or_default();
                    match status {
                        "open" => e.2 += 1,
                        "served" if class.starts_with("recipe") => e.1 += 1,
                        "served" => e.0 += 1,
                        _ => {}
                    }
                }
                if status == "open" {
                    open_keys.push(r["key"].clone());
                }
                if status == "served" {
                    *by_class.entry(class).or_default() += 1;
                    if src.starts_with("rf:") || src == "default-recipe" {
                        by_server
                            .entry(src)
                            .or_default()
                            .push(json!({"key": r["key"],
                            "evidence": r["evidence"], "basis": r["basis"]}));
                    }
                }
            }
        }
        let lines: Vec<bool> = self.nodes[i]
            .p1
            .as_ref()
            .map(|p| p.frame.mask().to_vec())
            .unwrap_or_default();
        // terminal proposals: open keys with their face and the steps of that face (the escalation's generic end)
        let mut proposals = vec![];
        if self.nodes[i].policy.proposals.enabled {
            for k in &open_keys {
                let key: Vec<i32> = serde_json::from_value(k.clone()).unwrap_or_default();
                let face = Face::of_key(&key, &lines).map(|f| f.label());
                let steps = face
                    .as_ref()
                    .and_then(|fl| d.faces.iter().find(|f| &f.face == fl))
                    .map(|f| f.recipe_steps.clone())
                    .unwrap_or_default();
                let known = self.nodes[i].policy.proposals.known_open.contains(&key);
                let status = if known {
                    "known_open"
                } else {
                    "terminal_proposal (no relation within the policy; awaiting an operator decision)"
                };
                proposals.push(
                    json!({"key": key, "face": face, "tried_steps": steps, "default_recipe": true,
                    "status": status}),
                );
            }
        }
        // the close-out by the proposals (withheld open declaration)
        let withheld: Vec<Event> = match read_input(st, &input_withheld(&n)) {
            Ok(b) => serde_json::from_slice::<Value>(&b)
                .ok()
                .and_then(|v| serde_json::from_value(v["events"].clone()).ok())
                .unwrap_or_default(),
            Err(_) => vec![],
        };
        let (closed, why) = if st.inputs.contains_key(&input_withheld(&n)) {
            self.closeout_check(i, &fin, open_keys.len())
        } else {
            (
                false,
                "no withheld open declaration (closed by the closing loop, or close-out off)"
                    .into(),
            )
        };
        let exp = format!("{dir}/export");
        if closed {
            // the statement with the proposals declared terminal (beside the closing loop's)
            let open_set: BTreeSet<String> = open_keys.iter().map(|k| k.to_string()).collect();
            let mut t = String::new();
            for l in &stmt_lines {
                let mut r: Value = serde_json::from_str(l).map_err(|e| e.to_string())?;
                if r["status"] == "open" && open_set.contains(&r["key"].to_string()) {
                    r["status"] = json!("terminal");
                    r["basis"] = json!("terminal_proposal_declared");
                }
                t.push_str(&r.to_string());
                t.push('\n');
            }
            rt.accepts.push(record(
                format!("{dir}/statement-with-proposals/statement.jsonl"),
                t.into_bytes(),
            ));
            rt.accepts.push(record(
                format!("{dir}/statement-with-proposals/terminals_declared.json"),
                json_bytes(&json!(open_keys)),
            ));
            let nr = st.nodes.get(&node).ok_or("node unknown")?;
            if let (Some(c), Some(dd)) = (&nr.state.closure, &nr.state.dispatch_order) {
                rt.events.push(Event::Node {
                    node: node.clone(),
                    ev: NodeEvent::ClosureResult {
                        round: nr.state.closure_rounds + 1,
                        defect_keys: 0,
                        closure_keys: c.closure_keys,
                        rules_used: None,
                        rules_total: None,
                        keyset: None,
                        rule_set_digest: nr.rule_set.digest(),
                        dispatch_order_digest: dd.clone(),
                        envelope: crate::ledger::EnvelopeKind::Measured,
                        envelope_tables: vec![],
                    },
                });
            }
            rt.events.push(Event::Note {
                text: format!(
                    "repairflow {node}: closed with {} declared terminal proposal(s): {why}",
                    open_keys.len()
                ),
            });
            rt.events.push(Event::Node {
                node: node.clone(),
                ev: NodeEvent::DeclareClosed,
            });
        } else {
            rt.events.extend(withheld);
        }
        // the export: symbolic table, recipes used (escalation order), certified keys per recipe, manifest, SHA256SUMS
        let mut sums = String::new();
        let mut recipes = vec![];
        let mut exported_recipe_bytes = 0u64;
        let mut exported_table_bytes = 0u64;
        if let Ok(b) = std::fs::read(self.abs(&format!("{dir}/symbolic.table.jsonl"))) {
            exported_table_bytes = b.len() as u64;
            sums.push_str(&format!("{}  symbolic.table.jsonl\n", sha256_hex(&b)));
            rt.accepts
                .push(record(format!("{exp}/symbolic.table.jsonl"), b));
        }
        let costs = self.solve_costs(st, i);
        let range = |v: &[f64]| -> Value {
            if v.is_empty() {
                return Value::Null;
            }
            let mut w = v.to_vec();
            w.sort_by(|a, b| a.total_cmp(b));
            json!({"min": w[0], "median": w[w.len() / 2], "max": w[w.len() - 1], "solves": w.len()})
        };
        let mut face_entries = vec![];
        for f in &d.faces {
            let face = Face::new(f.sd.clone(), f.sn.clone());
            let mut esc = vec![];
            for s in &f.recipe_steps {
                let id = format!("rf:{}:{s}", rf::face_tag(&face));
                let fname = format!("{}.{s}.json", rf::face_tag(&face));
                let certified = by_server.get(&id).cloned().unwrap_or_default();
                let Ok(b) = std::fs::read(self.abs(&format!("{dir}/recipes/{fname}"))) else {
                    continue;
                };
                let sha = sha256_hex(&b);
                let cost = costs.get(&id).map(|c| range(&c.2)).unwrap_or(Value::Null);
                esc.push(json!({"source": id, "step": s, "file": format!("recipes/{fname}"), "sha256": sha,
                    "certified_keys": certified.len(), "per_key_cpu_s": cost}));
                if certified.is_empty() {
                    continue;
                }
                exported_recipe_bytes += b.len() as u64;
                sums.push_str(&format!("{sha}  recipes/{fname}\n"));
                let cf: String = certified.iter().map(|c| format!("{c}\n")).collect();
                sums.push_str(&format!(
                    "{}  certified/{}.{s}.jsonl\n",
                    sha256_hex(cf.as_bytes()),
                    rf::face_tag(&face)
                ));
                rt.accepts.push(record(
                    format!("{exp}/certified/{}.{s}.jsonl", rf::face_tag(&face)),
                    cf.into_bytes(),
                ));
                recipes.push(json!({"source": id, "face": f.face, "step": s, "file": format!("recipes/{fname}"),
                    "sha256": sha, "certified_keys_served": certified.len(), "per_key_cpu_s": cost}));
                rt.accepts.push(record(format!("{exp}/recipes/{fname}"), b));
            }
            let pf = per_face.get(&f.face).copied().unwrap_or_default();
            let served = if f.exception.is_some() {
                if f.kept.is_some() {
                    "symbolic"
                } else {
                    "pointwise_via_plain_faces"
                }
            } else if pf.2 > 0 {
                "open"
            } else if pf.1 > 0 {
                "pointwise"
            } else {
                "symbolic"
            };
            // route 1 failed for SIZE / budget while route 2 served every key: a typed candidate for a larger-budget
            // symbolic attempt later (never "closed for all layers")
            let size_stop = match &f.symbolic.class {
                rf::Class::OverCap { .. }
                | rf::Class::CostlierThanRecipe
                | rf::Class::OverNodeBudget { .. } => true,
                rf::Class::NoCandidate { why } => {
                    why.contains("KilledTime") || why.contains("budget") || why.contains("refus")
                }
                _ => false,
            };
            let status = if f.kept.is_some() {
                json!("closed_symbolic_all_layers")
            } else if size_stop && pf.1 > 0 && pf.2 == 0 {
                json!({"candidate_larger_budget_symbolic": f.symbolic.class,
                    "note": "large with the current engine (unminimised seeds): a candidate for the minimised attempt, not evidence that no small rule exists"})
            } else if pf.1 > 0 && pf.2 == 0 {
                json!("pointwise_certified_where_solved")
            } else if pf.2 > 0 {
                json!("open")
            } else {
                json!("served_by_tables")
            };
            let note = if served == "pointwise" {
                "served POINTWISE: certified keys + run-time recipe solves for the rest (a key of this face may cost a solve)"
            } else {
                ""
            };
            face_entries.push(
                json!({"face": f.face, "sd": f.sd, "sn": f.sn, "escalation": esc,
                "symbolic_kept": f.kept.is_some(), "exception": f.exception,
                "domain_covered": f.domain_covered, "served": served,
                "keys_by": {"tables": pf.0, "recipes": pf.1, "open": pf.2}, "note": note,
                "status": status, "layers": f.layers,
                "evidence": "certified where solved (certified/ files); deeper layers: NOT sampled in this version; outside: run-time solve with the same escalation"}),
            );
        }
        // the default sequence (the closing loop's default recipe) with its certified keys and cost
        let dflt = self.nodes[i].cfg.default_recipe.clone();
        let mut default_seq = vec![];
        if let Some(pth) = &dflt
            && let Ok(b) = std::fs::read(pth)
        {
            let certified = by_server.get("default-recipe").cloned().unwrap_or_default();
            let cf: String = certified.iter().map(|c| format!("{c}\n")).collect();
            sums.push_str(&format!(
                "{}  recipes/default.recipe.json\n",
                sha256_hex(&b)
            ));
            sums.push_str(&format!(
                "{}  certified/default.jsonl\n",
                sha256_hex(cf.as_bytes())
            ));
            default_seq.push(json!({"source": "default-recipe", "file": "recipes/default.recipe.json",
                "sha256": sha256_hex(&b), "certified_keys": certified.len(),
                "per_key_cpu_s": costs.get("default-recipe").map(|c| range(&c.2)).unwrap_or(Value::Null)}));
            rt.accepts
                .push(record(format!("{exp}/recipes/default.recipe.json"), b));
            rt.accepts.push(record(
                format!("{exp}/certified/default.jsonl"),
                cf.into_bytes(),
            ));
        }
        let policy = &self.nodes[i].policy;
        let manifest = json!({
            "schema": MANIFEST_SCHEMA, "node": node,
            "node_order": self.nodes[i].cfg.solve.order,
            "descent": "every relation used is accepted only if each right-hand-side key is a hand-down (some line <= 0) or STRICTLY LOWER than the key under the node order (the engine's solve-recipe all_lower check, re-checked by the importer, interfaces/import.md 5.3): a chain of run-time solves strictly descends in a well-founded order on the keys of the sector and cannot cycle; its LENGTH is not bounded by this (dots may climb while the order descends): the run-time budgets below bound it, with a typed resource error",
            "relation_rule": "m1_first (recipe files are rustred.recipe.v1 in the v1.1 form without the field; the engine default)",
            "dispatch": "tables first (symbolic.table.jsonl after the node's own tables), then per face the escalation in order (first step that certifies a key), then the default sequence; a key outside every certified set is solved at run time with the same order",
            "faces": face_entries, "default_sequence": default_seq,
            "runtime": policy.runtime,
            "side_file_fields": {
                "escalation_order": "no field in rustred.recipe.v1: proposed `escalation: {group, rank}` on each recipe",
                "relation_rule": "v1.2 field exists in the engine; the campaign's recipe reader (core worklist::recipe::Recipe) refuses it today: REQUEST to read `relation_rule` and `domain.keys`",
                "runtime_budgets": "no field: proposed node-level `recipe_runtime` record in the package manifest",
                "descent_measure": "the node order is in each recipe's provenance (string); proposed explicit `node_order` field",
                "per_key_cost": "no field: proposed `cost_hint_s {min, median, max}` per recipe"
            },
            "symbolic_table": if exported_table_bytes > 0 { json!("symbolic.table.jsonl") } else { Value::Null },
            "policy_digest": d.policy_digest,
        });
        // rustred.relation-entry.v1 entries of the certified recipe relations (+ the d = 4 pole presence)
        let mut cert_set: BTreeSet<(String, Vec<i32>)> = BTreeSet::new();
        for (src, ks) in &by_server {
            for k in ks {
                if let Ok(key) = serde_json::from_value::<Vec<i32>>(k["key"].clone()) {
                    cert_set.insert((src.clone(), key));
                }
            }
        }
        let fam_text = self.nodes[i]
            .p1
            .as_ref()
            .and_then(|p| {
                p.frame_header["original_family_source"]
                    .as_str()
                    .map(str::to_string)
            })
            .unwrap_or_default();
        let order_canon = self.nodes[i].cfg.solve.order.to_string();
        let ident_src = json!({"family_blake3": blake3_hex(fam_text.as_bytes()),
            "order": serde_json::from_str::<Value>(&order_canon).unwrap_or(Value::Null),
            "arity": mask.len()});
        let node_identity = blake3_hex(ident_src.to_string().as_bytes());
        let (entries, rel_stats) =
            relation_entries(st, &node, &node_identity, &cert_set, &self.cdir);
        let ej: String = entries.iter().map(|e| format!("{e}\n")).collect();
        sums.push_str(&format!(
            "{}  relations.entries.jsonl\n",
            sha256_hex(ej.as_bytes())
        ));
        rt.accepts.push(record(
            format!("{exp}/relations.entries.jsonl"),
            ej.into_bytes(),
        ));
        // the common face record (face_recipe.v1 + our additive fields)
        let frs = rf::face_format::face_recipe_records(
            &d,
            &rf::face_format::Binding {
                node_identity: Some(node_identity.clone()),
                table_set_digest: None,
                engine_binary_sha256: None,
                policy_digest: d.policy_digest.clone(),
            },
        );
        let fj: String = frs.iter().map(|r| format!("{r}\n")).collect();
        sums.push_str(&format!(
            "{}  face_recipes.jsonl\n",
            sha256_hex(fj.as_bytes())
        ));
        rt.accepts
            .push(record(format!("{exp}/face_recipes.jsonl"), fj.into_bytes()));
        let mb = json_bytes(&manifest);
        sums.push_str(&format!("{}  manifest.json\n", sha256_hex(&mb)));
        rt.accepts.push(record(format!("{exp}/manifest.json"), mb));
        let rj: String = recipes.iter().map(|r| format!("{r}\n")).collect();
        sums.push_str(&format!("{}  recipes.jsonl\n", sha256_hex(rj.as_bytes())));
        rt.accepts
            .push(record(format!("{exp}/recipes.jsonl"), rj.into_bytes()));
        rt.accepts
            .push(record(format!("{exp}/SHA256SUMS"), sums.into_bytes()));
        // the S jobs' cost
        let (mut s_cpu, mut s_wall, mut s_jobs, mut s_peak) = (0.0f64, 0.0f64, 0u64, 0u64);
        for (_, _, jobs) in self.s_items(st, i) {
            for j in jobs {
                if let Some(rd) = st.jobs.get(&j).and_then(|x| x.run_dir.clone()) {
                    let c = super::termmin::cost_of(Path::new(&rd), &j);
                    s_jobs += 1;
                    s_cpu += c.cpu_s.unwrap_or(0.0);
                    s_wall += c.wall_s.unwrap_or(0.0);
                    s_peak = s_peak.max(c.vmhwm_kb.unwrap_or(0));
                }
            }
        }
        let sym_keys = by_class.get("repair-symbolic").copied().unwrap_or(0);
        let face_keys = by_class.get("recipe-face").copied().unwrap_or(0);
        let def_keys = by_server.get("default-recipe").map(Vec::len).unwrap_or(0);
        let rep = json!({
            "schema": REPORT_SCHEMA, "node": node,
            "policy": policy, "policy_digest": d.policy_digest,
            "faces": d.faces, "by_class_symbolic": d.by_class,
            "kept_rules": d.kept_rules, "kept_bytes": d.kept_bytes,
            "keys": {"by_rule_class": by_class, "symbolic": sym_keys, "face_recipe": face_keys,
                     "default_recipe": def_keys, "open": open_keys.len(),
                     "terminal_proposals": proposals.len()},
            "default_sequence_while_closing": {"keys": def_keys,
                "per_key_cpu_s": costs.get("default-recipe").map(|c| range(&c.2)).unwrap_or(Value::Null)},
            "recipes_used": recipes, "terminal_proposals": proposals,
            "closeout": {"closed_by_declared_proposals": closed, "why": why},
            "relation_entries": {"count": entries.len(), "node_identity": node_identity, "d4": rel_stats},
            "exported_bytes": {"symbolic_table": exported_table_bytes, "recipes": exported_recipe_bytes},
            "s_jobs": {"jobs": s_jobs, "cpu_s": s_cpu, "wall_s": s_wall, "peak_kb": s_peak},
            "solve_costs_by_source": costs.iter().map(|(k, v)| (k.clone(), json!({"jobs": v.0, "keys": v.1, "per_key_cpu_s": range(&v.2)}))).collect::<BTreeMap<_, _>>(),
            "closeloop_final": {"verdict": fin["verdict"], "keys": fin["keys"], "economy": fin["economy"], "blockers": fin["blockers"]},
            "outside_chart_open": d.outside_chart.len(),
        });
        let rec = record(format!("{dir}/report.json"), json_bytes(&rep));
        self.reg(rt, input_report(&n), ROLE_RESULT, rec);
        Ok(())
    }
}

/// The close-out by terminal proposals (user ruling 2026-11-14): closed iff the closing loop's ONLY blockers are its
/// open keys (`open_keys:` / `import_open_keys:`), every closure key but those is certified, and their number is
/// within `max_terminals`. Declaring an open key terminal stops its dispatch only (an open key has no expansion), so
/// the closure is unchanged otherwise. Returns (closed, why).
pub fn closeout_check(p: &rf::policy::ProposalPolicy, fin: &Value, open: usize) -> (bool, String) {
    let blockers: Vec<String> = fin["blockers"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|x| x.as_str().map(str::to_string))
        .collect();
    let others: Vec<&String> = blockers
        .iter()
        .filter(|b| !b.starts_with("import_open_keys:") && !b.starts_with("open_keys:"))
        .collect();
    let k = &fin["keys"];
    let (closure, cert) = (
        k["closure"].as_u64().unwrap_or(0),
        k["certified"].as_u64().unwrap_or(0),
    );
    if !p.enabled || !p.declare_terminal {
        return (false, "declaration of proposals off by policy".into());
    }
    if fin["verdict"] != "open" {
        return (false, format!("verdict {}", fin["verdict"]));
    }
    if !others.is_empty() {
        return (false, format!("other blockers: {others:?}"));
    }
    if open == 0 || open > p.max_terminals {
        return (false, format!("{open} proposals (max {})", p.max_terminals));
    }
    if cert + open as u64 != closure {
        return (
            false,
            format!("certified {cert} + proposals {open} != closure {closure}"),
        );
    }
    (
        true,
        format!("{open} terminal proposals declared; every other closure key certified"),
    )
}

/// `{"type":"rule","index":K, <the other fields sorted>}`: rr_proof reads a record at an offset only when its line
/// starts with `{"type":"rule"`; the index is the ordinal in the re-homed table (labels unique per file).
fn rehome_line(line: &str, index: usize) -> Result<String, String> {
    let v: Value = serde_json::from_str(line).map_err(|e| format!("kept record: {e}"))?;
    let o = v.as_object().ok_or("kept record: not an object")?;
    let mut s = format!("{{\"type\":\"rule\",\"index\":{index}");
    let mut keys: Vec<&String> = o.keys().filter(|k| *k != "type" && *k != "index").collect();
    keys.sort();
    for k in keys {
        s.push(',');
        s.push_str(&serde_json::to_string(k).expect("json"));
        s.push(':');
        s.push_str(&serde_json::to_string(&o[k]).expect("json"));
    }
    s.push('}');
    Ok(s)
}

fn class_text(c: &rf::Class) -> String {
    serde_json::to_string(c).unwrap_or_else(|_| "?".into())
}

/// The fake child's script for a face (tests): a rule record, or a stop.
fn fake_gen_script(f: &FakeGen, face: &str) -> FakeScript {
    let mut ops = vec![FakeOp::Prepare { ms: 5 }];
    match f.faces.get(face) {
        Some(b) if b.get("rule").is_some() => {
            ops.push(FakeOp::Case {
                ordinal: 0,
                ms: f.ms,
                work: BTreeMap::new(),
                result: CaseResult::Rule,
                rule: Some(FakeRule {
                    record: b["rule"].clone(),
                    loci: None,
                }),
                children: vec![],
            });
            ops.push(FakeOp::End {
                stop: None,
                verdict: None,
            });
        }
        Some(b) => {
            let stop: StopCode =
                serde_json::from_value(b["stop"].clone()).unwrap_or(StopCode::OpenObligations);
            ops.push(FakeOp::Case {
                ordinal: 0,
                ms: f.ms,
                work: BTreeMap::new(),
                result: if stop == StopCode::BudgetWall {
                    CaseResult::Budget
                } else {
                    CaseResult::Refused
                },
                rule: None,
                children: vec![],
            });
            ops.push(FakeOp::End {
                stop: Some(stop),
                verdict: None,
            });
        }
        None => ops.push(FakeOp::End {
            stop: Some(StopCode::OpenObligations),
            verdict: None,
        }),
    }
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

/// ROUTE 0 (pointwise): the node's spec with the verified symmetry-credit source added (when the policy asks and the
/// spec has none). Returns (spec text, the added source if any).
fn with_route0(text: &str, policy: &RepairPolicy) -> Result<(String, Option<Value>), String> {
    let text = with_composites(text, policy)?;
    let text = text.as_str();
    if !policy.route0.symmetry_credit {
        return Ok((text.to_string(), None));
    }
    let mut v: Value = serde_json::from_str(text).map_err(|e| format!("import spec: {e}"))?;
    let srcs = v
        .get_mut("sources")
        .and_then(Value::as_array_mut)
        .ok_or("import spec: no sources")?;
    if srcs.iter().any(|s| s["kind"] == "symmetry_credit") {
        return Ok((text.to_string(), None));
    }
    let src = match &policy.route0.family {
        Some(f) => {
            json!({"id": "rf:symmetry-credit", "kind": "symmetry_credit", "class": "symmetry-credit",
                          "family": {"path": f}})
        }
        None => {
            json!({"id": "rf:symmetry-credit", "kind": "symmetry_credit", "class": "symmetry-credit",
                       "group_format": "derive"})
        }
    };
    srcs.push(src.clone());
    Ok((
        serde_json::to_string_pretty(&v).expect("json") + "\n",
        Some(src),
    ))
}

/// The pinned symmetry-composite result of the policy (`route0.composite_input`) as a `supplied_relations` source of
/// the spec (once; placed before the default recipe): the import loads it, re-checks descent, cites the verification.
fn with_composites(text: &str, policy: &RepairPolicy) -> Result<String, String> {
    if policy.route0.composite_input.is_none() && policy.evidence.facts.is_empty() {
        return Ok(text.to_string());
    }
    let mut v: Value = serde_json::from_str(text).map_err(|e| format!("import spec: {e}"))?;
    // the typed zero-fact slot: extra fact files appended to the spec's evidence (once)
    if !policy.evidence.facts.is_empty() {
        let ev = v
            .as_object_mut()
            .ok_or("import spec: not an object")?
            .entry("evidence")
            .or_insert_with(|| json!({}));
        let facts = ev
            .as_object_mut()
            .ok_or("import spec: evidence not an object")?
            .entry("facts")
            .or_insert_with(|| json!([]));
        let arr = facts.as_array_mut().ok_or("import spec: evidence.facts")?;
        for f in &policy.evidence.facts {
            if !arr.iter().any(|x| x["path"] == f.path.as_str()) {
                arr.push(json!({"path": f.path, "sha256": f.sha256}));
            }
        }
    }
    let Some(ci) = &policy.route0.composite_input else {
        return Ok(serde_json::to_string_pretty(&v).expect("json") + "\n");
    };
    let srcs = v
        .get_mut("sources")
        .and_then(Value::as_array_mut)
        .ok_or("import spec: no sources")?;
    if srcs.iter().any(|s| s["id"] == "rf:symmetry-composite") {
        return Ok(serde_json::to_string_pretty(&v).expect("json") + "\n");
    }
    let at = srcs
        .iter()
        .position(|s| s["id"] == "default-recipe")
        .unwrap_or(srcs.len());
    srcs.insert(
        at,
        json!({"id": "rf:symmetry-composite", "kind": "supplied_relations", "class": "symmetry-composite",
            "path": ci.table.path, "sha256": ci.table.sha256,
            "verification": {"path": ci.verification.path, "sha256": ci.verification.sha256}}),
    );
    Ok(serde_json::to_string_pretty(&v).expect("json") + "\n")
}

/// P1: load + check the node's effective spec; faces of the uncovered open keys; shadowing test; the check record.
fn p1_pass(
    node: &str,
    text: &str,
    base: &Path,
    policy: &RepairPolicy,
    threads: usize,
    rows: &Path,
) -> Result<P1, String> {
    let (text, route0) = with_route0(text, policy)?;
    let text = text.as_str();
    let spec = TableSetSpec::from_json(text).map_err(|e| format!("import spec: {e}"))?;
    let set = rustred_campaign_core::import::load(&spec, base)
        .map_err(|e| format!("import load: {e}"))?;
    let ev = Evidence::from_spec(&set, base).map_err(|e| format!("evidence: {e}"))?;
    let res = rustred_campaign_core::import::check::check_streamed(
        &set,
        &ev,
        &CheckOptions {
            threads,
            tier: None,
            max_rounds: None,
        },
        rows,
    )
    .map_err(|e| format!("import check: {e}"))?;
    // keys served by a verified symmetry credit (route 0, pointwise), by support signature
    let mut route0_keys: BTreeMap<String, u64> = BTreeMap::new();
    for r in &res.rows {
        if r.status == RowStatus::Served
            && matches!(
                r.server,
                Some(rustred_campaign_core::import::check::Server::Credit { .. })
            )
            && let Some(f) = Face::of_key(&r.key, set.frame.mask())
        {
            *route0_keys.entry(f.label()).or_default() += 1;
        }
    }
    let mut uncovered: Vec<Vec<i32>> = vec![];
    let mut other_open: BTreeMap<String, u64> = BTreeMap::new();
    for r in &res.rows {
        if r.status != RowStatus::Open {
            continue;
        }
        let s = r.open_status.clone().unwrap_or_else(|| "open".into());
        if s == "uncovered" {
            uncovered.push(r.key.clone());
        } else {
            *other_open.entry(s).or_default() += 1;
        }
    }
    let lines = set.frame.mask().to_vec();
    let (mut plain, outside) = rf::group_open_keys(&uncovered, &lines, &policy.symbolic.face_order);
    // EXCEPTION FACES first (the primary object): (rule, branch) from the domain code, observed keys = evidence
    let src = rustred_campaign_core::partition::DispatcherSource::new(&set.disp);
    let (xfaces, attributed, astats) = if policy.faces.exception_faces {
        let prov = |entry: usize| {
            let r = src.rule_ref(entry);
            let info = &set.rules[r.table][r.record];
            (info.source.clone(), info.label, info.record_sha256.clone())
        };
        rf::exception_faces(
            &uncovered,
            &src,
            &lines,
            policy.faces.max_attribution_evaluations,
            &prov,
        )
    } else {
        (vec![], BTreeSet::new(), rf::AttributionStats::default())
    };
    for f in &mut plain {
        f.unattributed = f
            .open_keys
            .iter()
            .filter(|k| !attributed.contains(*k))
            .count() as u64;
    }
    let faces: Vec<FaceGroup> = xfaces.into_iter().chain(plain).collect();
    // frame header (raw first line of the frame_from source) and the node order's coordinate priority
    let fpath = rf::symflow::frame_table_path(&set).ok_or("frame_from: no table file")?;
    let fpath = if Path::new(&fpath).is_absolute() {
        PathBuf::from(&fpath)
    } else {
        base.join(&fpath)
    };
    let header_line = {
        use std::io::BufRead;
        let f = std::fs::File::open(&fpath).map_err(|e| format!("{}: {e}", fpath.display()))?;
        let mut l = String::new();
        std::io::BufReader::new(f)
            .read_line(&mut l)
            .map_err(|e| e.to_string())?;
        l.trim_end_matches('\n').to_string()
    };
    let header: Value =
        serde_json::from_str(&header_line).map_err(|e| format!("frame header: {e}"))?;
    let cp: Option<Vec<usize>> = header["node_order"]["coordinate_priority"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_u64().map(|x| x as usize))
                .collect()
        });
    let (mut lp, mut np) = rustred_campaign_core::termmin::priorities(&lines, cp.as_deref());
    if let Some(l) = &policy.recipes.line_priority {
        lp = l.clone();
    }
    if let Some(nn) = &policy.recipes.numerator_priority {
        np = nn.clone();
    }
    // ROUTE 0b: the smallest DESCENDING multi-term symmetry relation at each open key (every verified element alone;
    // descent re-checked under the FULL node comparator); served keys leave the symbolic step
    let mut faces = faces;
    let mut r0b = json!({"enabled": policy.route0.symmetry_relation});
    let mut r0b_table = None;
    if policy.route0.symmetry_relation {
        let gs: rustred_campaign_core::import::symmetry::GroupSpec = match &policy.route0.family {
            Some(f) => serde_json::from_value(json!({"family": {"path": f}})),
            None => serde_json::from_value(json!({})),
        }
        .map_err(|e| e.to_string())?;
        match rustred_campaign_core::import::symmetry::load_verified_group(
            &gs,
            base,
            set.frame.mask(),
            set.cmp.presentation().new_to_old(),
            header["family_source"].as_str(),
        ) {
            Err(e) => r0b["skipped"] = json!(format!("no verified group: {e}")),
            Ok((g, grep)) => {
                let cmp = |a: &[i32], b: &[i32]| set.cmp.cmp(a, b);
                let mut served: BTreeSet<Vec<i32>> = BTreeSet::new();
                let mut lines_out: Vec<String> = vec![];
                let mut regions: BTreeMap<String, BTreeMap<usize, u64>> = BTreeMap::new();
                let (mut not_lower, mut unknown, mut none, mut tried) = (0u64, 0u64, 0u64, 0u64);
                for k in uncovered.iter().take(policy.route0.max_keys as usize) {
                    tried += 1;
                    match rf::symrel::smallest_symmetry_relation(
                        &rustred_campaign_core::independent_algebra::Independent,
                        &g,
                        k,
                        &cmp,
                        policy.route0.max_terms,
                    ) {
                        Err(_) => unknown += 1,
                        Ok(r) => match r.best {
                            None => none += 1,
                            Some(b) => {
                                // acceptance: EVERY right-hand-side key strictly lower under the full comparator
                                if !b
                                    .rhs
                                    .iter()
                                    .all(|(q, _)| cmp(q, k) == std::cmp::Ordering::Less)
                                {
                                    not_lower += 1;
                                    continue;
                                }
                                let rhs: Vec<Value> = b
                                    .rhs
                                    .iter()
                                    .map(|(q, c)| {
                                        json!([
                                            rustred_campaign_core::termmin::key_text(q),
                                            rustred_campaign_core::termmin::symrel::q_text(c)
                                        ])
                                    })
                                    .collect();
                                lines_out.push(
                                    json!({"type": "rule", "chart": "", "index": lines_out.len(),
                                        "kind": "symmetry-relation", "case": "point", "fixed": k, "equations": [],
                                        "target": rustred_campaign_core::termmin::key_text(k), "rhs": rhs,
                                        "exceptions": [], "dispatch": "Point", "witness_loci": [],
                                        "symmetry_elements": [b.element]})
                                    .to_string(),
                                );
                                let sig = Face::of_key(k, set.frame.mask())
                                    .map(|f| f.label())
                                    .unwrap_or_default();
                                *regions
                                    .entry(sig)
                                    .or_default()
                                    .entry(b.element)
                                    .or_default() += 1;
                                served.insert(k.clone());
                            }
                        },
                    }
                }
                // HAND-DOWNS: right-hand-side keys of the kept relations that are themselves open, by face (the faces
                // are coupled: a face is served when the closing loop's fixed point serves those keys; no cycle: every
                // kept relation strictly descends)
                let open_set: BTreeSet<&Vec<i32>> = uncovered.iter().collect();
                let mut hands: BTreeMap<String, BTreeMap<String, u64>> = BTreeMap::new();
                for l in &lines_out {
                    let v: Value = serde_json::from_str(l).unwrap_or(Value::Null);
                    let k: Vec<i32> =
                        serde_json::from_value(v["fixed"].clone()).unwrap_or_default();
                    let from = Face::of_key(&k, set.frame.mask())
                        .map(|f| f.label())
                        .unwrap_or_default();
                    for t in v["rhs"].as_array().into_iter().flatten() {
                        let txt = t[0].as_str().unwrap_or("");
                        let q: Vec<i32> = txt
                            .trim_start_matches("I(")
                            .trim_end_matches(')')
                            .split(',')
                            .filter_map(|x| x.trim().parse().ok())
                            .collect();
                        if open_set.contains(&q) && !served.contains(&q) {
                            let to = Face::of_key(&q, set.frame.mask())
                                .map(|f| f.label())
                                .unwrap_or_default();
                            *hands
                                .entry(from.clone())
                                .or_default()
                                .entry(to)
                                .or_default() += 1;
                        }
                    }
                }
                let r0b_hands = json!(hands);
                for f in &mut faces {
                    f.open_keys.retain(|k| !served.contains(k));
                    if f.exception.is_none() {
                        f.unattributed = f
                            .open_keys
                            .iter()
                            .filter(|k| !attributed.contains(*k))
                            .count() as u64;
                    }
                }
                faces.retain(|f| !f.open_keys.is_empty());
                if !lines_out.is_empty() {
                    let mut t = json!({"type": "header", "schema": "table_dump.v1", "family": "repairflow-route0b"})
                        .to_string();
                    t.push('\n');
                    for l in &lines_out {
                        t.push_str(l);
                        t.push('\n');
                    }
                    r0b_table = Some(t);
                }
                r0b = json!({"enabled": true, "group": {"elements": grep.elements, "digest": grep.digest},
                    "tried": tried, "served": served.len(), "no_relation": none, "not_lower": not_lower,
                    "unknown": unknown, "regions_by_signature_and_element": regions,
                    "acyclic": "by construction: every kept relation strictly descends under the node comparator (re-checked per key)",
                    "hands_down_to": r0b_hands});
            }
        }
    }
    // shadowing test per PLAIN face (against the in-force tables, dispatch order); an exception face is cut out of a
    // rule by construction (its keys are open: not matched first by an earlier entry)
    let budget = rustred_campaign_core::loci::contain::ProofBudget::default();
    let shadow: Vec<ShadowClass> = faces
        .iter()
        .map(|f| {
            if policy.shadow_test && f.exception.is_none() {
                rf::shadow_face(&f.face(), &src, &set.frame, &budget)
            } else {
                ShadowClass::Skipped
            }
        })
        .collect();
    let rec = json!({
        "schema": CHECK_SCHEMA, "node": node, "policy_digest": policy.digest(),
        "spec_digest": res.spec_digest, "closure_keys": res.summary.keys_total, "closure_open": res.summary.open_total,
        "open_uncovered": uncovered.len(), "open_other": other_open,
        "faces": faces.iter().zip(&shadow).map(|(f, s)| json!({"face": f.label, "sd": f.sd, "sn": f.sn,
            "open_keys": f.open_keys, "shadow": s})).collect::<Vec<_>>(),
        "outside_chart": outside, "line_priority": lp, "numerator_priority": np,
        "attribution": astats,
        "route0b": r0b,
        "route0": {"source": route0, "credit_served_keys_by_signature": route0_keys},
        "frame_header_sha256": sha256_hex(header_line.as_bytes()),
    });
    let family = rf::faces::job_family_from_header(&header).ok();
    Ok(P1 {
        r0b_table,
        family,
        faces,
        shadow,
        outside,
        frame: set.frame.clone(),
        frame_header: header,
        frame_header_line: header_line,
        line_prio: lp,
        num_prio: np,
        record: json_bytes(&rec),
    })
}

impl<S: WorkSource> WorkSource for RepairFlowSource<S> {
    fn bootstrap(&mut self, st: &CampaignState, ctx: &SourceContext) -> Result<Vec<Event>, String> {
        let mut ev = self.inner.bootstrap(st, ctx)?;
        for r in &self.nodes {
            let id = input_policy(&r.n);
            let b = json_bytes(&json!({"schema": rf::POLICY_SCHEMA, "node": r.cfg.node,
                "policy_digest": r.policy.digest(), "policy": r.policy}));
            match st.inputs.get(&id) {
                Some(x) if x.blake3 != blake3_hex(&b) => {
                    return Err(format!(
                        "node {}: the repair policy changed since the campaign started: refused; start a new campaign directory",
                        r.cfg.node
                    ));
                }
                Some(_) => {}
                None => {
                    let rel = format!("{}/policy.json", rf_dir(&r.n));
                    let p = self.abs(&rel);
                    if let Some(d) = p.parent() {
                        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
                    }
                    crate::atomic::write_atomic(&p, &b)
                        .map_err(|e| format!("{}: {e}", p.display()))?;
                    ev.push(Event::InputRegistered {
                        input: id,
                        role: ROLE_POLICY.into(),
                        path: p.display().to_string(),
                        bytes: b.len() as u64,
                        blake3: blake3_hex(&b),
                    });
                }
            }
        }
        Ok(ev)
    }

    fn ready(&mut self, st: &CampaignState, ctx: &SourceContext, limit: usize) -> Vec<JobRequest> {
        let mut out = vec![];
        for i in 0..self.nodes.len() {
            if self.nodes[i].p1.is_none()
                || st.inputs.contains_key(&input_extension(&self.nodes[i].n))
            {
                continue;
            }
            // PRE-FLIGHT: one job first; a frame mismatch is ONE error for the node (attention), never N refusals
            let items = self.s_items(st, i);
            let n = self.nodes[i].n.clone();
            let answered = items.iter().any(|(it, _, jobs)| {
                jobs.iter()
                    .any(|j| st.inputs.contains_key(&input_answer(it, j)))
            });
            let frame_err = items.iter().any(
                |(_, ph, _)| matches!(ph, ItemPhase::Attention(c) if c == "repair_frame_mismatch"),
            );
            if frame_err {
                continue;
            }
            let first = items.iter().map(|(it, _, _)| it.clone()).min();
            let _ = &n;
            for (item, ph, jobs) in items {
                if !answered && Some(&item) != first.as_ref() {
                    continue;
                }
                let job = match ph {
                    ItemPhase::Open => Some(format!("{item}-t{}", jobs.len())),
                    ItemPhase::InProgress => {
                        match jobs.last().and_then(|l| st.jobs.get(l)).map(|r| r.state) {
                            Some(AttemptState::Queued) => jobs.last().cloned(),
                            _ => None,
                        }
                    }
                    _ => None,
                };
                let Some(job) = job else { continue };
                let Some(face) = self.face_of(st, i, &item) else {
                    continue;
                };
                let prio = 1_000_000_000
                    - st.items
                        .get(&item)
                        .map(|x| x.weight as i64)
                        .unwrap_or(0)
                        .min(1);
                match self.job_request(i, &item, job, &face, prio) {
                    Ok(r) => out.push(r),
                    Err(e) => eprintln!("repairflow: no job for {item}: {e}"),
                }
            }
        }
        out.extend(self.inner.ready(st, ctx, limit));
        out.sort_by(|a, b| b.priority.cmp(&a.priority).then(a.job.cmp(&b.job)));
        out.truncate(limit);
        out
    }

    fn route(
        &mut self,
        st: &CampaignState,
        ctx: &SourceContext,
        done: &Settled,
    ) -> Result<Routing, String> {
        let Some(i) = self.node_of_item(st, &done.item) else {
            return self.inner.route(st, ctx, done);
        };
        let mut rt = Routing::default();
        let face = self
            .face_of(st, i, &done.item)
            .ok_or_else(|| format!("repairflow: {}: face unknown (P1 not loaded)", done.item))?;
        let p1 = self.nodes[i].p1.clone().ok_or("p1 not loaded")?;
        let c = &done.classification;
        let why = format!(
            "row {} {} {:?} {:?}: {}",
            c.row, c.sub, c.kind, c.action, c.why
        );
        let answer = match (&done.table, done.canonical_lines.is_empty()) {
            (Some(t), false) => {
                let run_dir = st
                    .jobs
                    .get(&done.job)
                    .and_then(|j| j.run_dir.clone())
                    .unwrap_or_default();
                match candidates_of(&run_dir, t, &done.canonical_lines, &p1, &face) {
                    Ok(Ok(cands)) => FaceAnswer::Candidates {
                        job: done.job.clone(),
                        candidates: cands,
                    },
                    Ok(Err((kind, w))) if kind == "frame" => FaceAnswer::FrameMismatch {
                        job: done.job.clone(),
                        why: w,
                    },
                    Ok(Err((_, w))) => FaceAnswer::DomainError {
                        job: done.job.clone(),
                        why: w,
                    },
                    Err(e) => FaceAnswer::NoCandidate {
                        job: done.job.clone(),
                        why: format!("{why}; {e}"),
                    },
                }
            }
            _ => FaceAnswer::NoCandidate {
                job: done.job.clone(),
                why,
            },
        };
        let has =
            matches!(&answer, FaceAnswer::Candidates { candidates, .. } if !candidates.is_empty());
        // a contract-level refusal / crash says nothing about the face: attention (operator retry / skip), never a
        // decision
        let infra = matches!(
            c.kind,
            OutcomeKind::Refused
                | OutcomeKind::UsageError
                | OutcomeKind::SetupError
                | OutcomeKind::Crashed
                | OutcomeKind::UnverifiableKind
        );
        let rec = record(
            format!(
                "{}/answers/{}.{}.answer.json",
                rf_dir(&self.nodes[i].n),
                done.item,
                done.job
            ),
            json_bytes(
                &json!({"schema": ANSWER_SCHEMA, "item": done.item, "face": face.label,
                "job": done.job, "attempt": done.attempt, "answer": answer}),
            ),
        );
        self.reg(
            &mut rt,
            input_answer(&done.item, &done.job),
            ROLE_ANSWER,
            rec,
        );
        if matches!(answer, FaceAnswer::FrameMismatch { .. }) {
            rt.events.push(Event::Item {
                item: done.item.clone(),
                ev: ItemEvent::Attention {
                    code: "repair_frame_mismatch".into(),
                    text: format!(
                        "repairflow: the generated table is not in the node's frame ({}): no further symbolic job for this node until the input is fixed",
                        match &answer {
                            FaceAnswer::FrameMismatch { why, .. } => why.as_str(),
                            _ => "",
                        }
                    ),
                },
            });
        } else if infra && !has {
            rt.events.push(Event::Item {
                item: done.item.clone(),
                ev: ItemEvent::Attention {
                    code: "repair_symbolic_refused".into(),
                    text: format!(
                        "repairflow: the generate job was refused ({}): operator retry or skip",
                        c.why
                    ),
                },
            });
        } else if has && let Some(t) = &done.table {
            rt.events.push(Event::Item {
                item: done.item.clone(),
                ev: ItemEvent::Produce {
                    job: done.job.clone(),
                    attempt: done.attempt,
                    table: t.clone(),
                },
            });
        } else {
            rt.events.push(Event::Item {
                item: done.item.clone(),
                ev: ItemEvent::Escalate {
                    to: Rung(1),
                    reason: "repairflow: no symbolic candidate (face goes to recipes)".into(),
                },
            });
        }
        Ok(rt)
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
                v => Err(format!("verb {v} not supported by the repair flow")),
            };
        }
        self.inner.control(st, verb, target)
    }

    fn poll(&mut self, st: &CampaignState, ctx: &SourceContext) -> Result<Routing, String> {
        let inner = self.inner.poll(st, ctx)?;
        let mut rt = self.filter_inner(inner);
        for i in 0..self.nodes.len() {
            let n = self.nodes[i].n.clone();
            let node = self.nodes[i].cfg.node.clone();
            let Some(nr) = st.nodes.get(&node) else {
                continue;
            };
            if nr.state.phase == NodePhase::Registered {
                continue;
            }
            if st.inputs.contains_key(&input_extension(&n)) {
                if st.inputs.contains_key(&input_final(&n))
                    && !st.inputs.contains_key(&input_report(&n))
                {
                    // P5 (the faces of the decisions; P1 not needed beyond the frame mask)
                    if self.nodes[i].p1.is_none() && self.nodes[i].pass.is_none() {
                        self.start_p1(st, i)?;
                    }
                    if let Some(h) = &self.nodes[i].pass
                        && h.is_finished()
                    {
                        let h = self.nodes[i].pass.take().expect("pass");
                        let p = h.join().map_err(|_| "p1 thread panicked".to_string())??;
                        self.nodes[i].p1 = Some(Arc::new(p));
                    }
                    if self.nodes[i].p1.is_some() {
                        self.report(st, i, &mut rt)?;
                    }
                }
                continue;
            }
            // P1
            if self.nodes[i].p1.is_none() {
                if self.nodes[i].pass.is_none() {
                    if !st.inputs.contains_key(&input_import_spec(&n)) {
                        continue;
                    }
                    self.start_p1(st, i)?;
                }
                let fin = self.nodes[i].pass.as_ref().is_some_and(|h| h.is_finished());
                if !fin {
                    continue;
                }
                let h = self.nodes[i].pass.take().expect("pass");
                let p = h.join().map_err(|_| "p1 thread panicked".to_string())??;
                match st.inputs.get(&input_check(&n)) {
                    Some(x) if x.blake3 != blake3_hex(&p.record) => {
                        return Err(format!(
                            "node {node}: the recomputed repair check differs from the recorded one: refused (U-1)"
                        ));
                    }
                    Some(_) => {}
                    None => {
                        if let Some(t) = &p.r0b_table {
                            rt.accepts.push(record(
                                format!("{}/symrel.table.jsonl", rf_dir(&n)),
                                t.clone().into_bytes(),
                            ));
                        }
                        let rec = record(format!("{}/check.json", rf_dir(&n)), p.record.clone());
                        self.reg(&mut rt, input_check(&n), ROLE_CHECK, rec);
                        let plan = rf::s_plan(&p.faces, &p.shadow, &self.nodes[i].policy);
                        for (k, (f, pl)) in p.faces.iter().zip(&plan).enumerate() {
                            if pl.is_some() {
                                continue;
                            }
                            let item = s_item(&n, k);
                            if !st.items.contains_key(&item) {
                                rt.events.push(Event::ItemCreated {
                                    item,
                                    node: node.clone(),
                                    kind: KIND_SYMBOLIC.into(),
                                    rung: Rung(0),
                                    parent: None,
                                    inputs: vec![input_check(&n)],
                                    label: f.label.clone(),
                                    weight: f.open_keys.len() as u64,
                                });
                            }
                        }
                    }
                }
                self.nodes[i].p1 = Some(Arc::new(p));
                // the barrier is evaluated on the next tick (the new items / check are not in `st` yet)
                continue;
            }
            if st.inputs.contains_key(&input_check(&n)) {
                self.barrier(st, i, &mut rt)?;
            }
        }
        Ok(rt)
    }

    fn confirm_coverage(
        &mut self,
        st: &CampaignState,
        ctx: &SourceContext,
        done: &Settled,
    ) -> Result<Option<Vec<Event>>, String> {
        if self.node_of_item(st, &done.item).is_some() {
            return Ok(None);
        }
        self.inner.confirm_coverage(st, ctx, done)
    }

    fn finished(&self, st: &CampaignState) -> bool {
        self.inner.finished(st)
            && self.nodes.iter().all(|r| {
                !st.inputs.contains_key(&input_final(&r.n))
                    || st.inputs.contains_key(&input_report(&r.n))
            })
    }

    fn outcome(&self, st: &CampaignState) -> crate::supervisor::worksource::CampaignOutcome {
        // the closing loop's verdict (the decorator adds no verdict of its own)
        self.inner.outcome(st)
    }

    fn describe(&self) -> String {
        if self.nodes.is_empty() {
            self.inner.describe()
        } else {
            format!(
                "{}; repair flow on {} node(s)",
                self.inner.describe(),
                self.nodes.len()
            )
        }
    }
}

type CandResult = Result<Vec<CandidateAnswer>, (String, String)>;

/// The candidate rule lines of a settled generate attempt: the RAW lines of the sealed table (re-read at the run
/// directory, BLAKE3 equal to the verified figures), each bound to a verified canonical line; the frame check of the
/// header (identity fields equal to the node's frame header; the fake child's own header is accepted as such and
/// marked); the measures and the face's open keys each candidate serves.
fn candidates_of(
    run_dir: &str,
    t: &crate::ledger::FileRef,
    canonical: &[String],
    p1: &P1,
    face: &FaceGroup,
) -> Result<CandResult, String> {
    let path = Path::new(run_dir).join("out").join("F.table.jsonl");
    let b = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    if blake3_hex(&b) != t.blake3 {
        return Err(format!(
            "{}: differs from the verified table",
            path.display()
        ));
    }
    let canon: BTreeSet<&str> = canonical.iter().map(String::as_str).collect();
    let text = String::from_utf8(b).map_err(|e| e.to_string())?;
    let mut out = vec![];
    for (no, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let c = rustred_campaign_contract::canon::canonical_line(line.as_bytes())
            .map_err(|e| format!("line {}: {e:?}", no + 1))?;
        let cs = String::from_utf8(c).map_err(|e| e.to_string())?;
        if !canon.contains(cs.trim_end_matches('\n')) && !canon.contains(cs.as_str()) {
            return Err(format!("line {}: not a verified canonical line", no + 1));
        }
        let v: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
        match v["type"].as_str() {
            Some("header") => {
                let fake = v["schema"].as_str() == Some("rustred.fake.table.v1");
                if !fake {
                    // `family_blake3` (the job-mode engine's digest of its family INPUT FILE) is provenance: the family
                    // TEXT (`family_source`) is compared (REQUEST to the core owner: add it to HEADER_PROVENANCE_FIELDS)
                    let mut a = rustred_campaign_core::assemble::header_identity(&v);
                    let mut f = rustred_campaign_core::assemble::header_identity(&p1.frame_header);
                    a.remove("family_blake3");
                    f.remove("family_blake3");
                    if a != f {
                        let k = a
                            .keys()
                            .chain(f.keys())
                            .find(|k| a.get(*k) != f.get(*k))
                            .cloned();
                        return Ok(Err((
                            "frame".into(),
                            format!("header identity differs at {k:?}"),
                        )));
                    }
                }
            }
            Some("rule") => {
                let m = rf::measures_of(line)?;
                let served = match rf::served_keys(line, &p1.frame, &face.open_keys) {
                    Ok(s) => s,
                    Err(e) => return Ok(Err(("domain".into(), e))),
                };
                // INTERIOR / BOUNDARY: the face keys outside the candidate's domain, by the domain's verdict (a boundary
                // sub-face; its keys fall to the face's recipes, or to a later candidate)
                let mut boundary: BTreeMap<String, u64> = BTreeMap::new();
                if let Ok(rt) =
                    rustred_campaign_core::accept::rule::RuleText::parse(line.as_bytes(), &p1.frame)
                    && let Some(dm) = rt.domain
                {
                    for k in face.open_keys.iter().filter(|k| !served.contains(*k)) {
                        let why = match dm.applies(k) {
                            rustred_campaign_core::domain::Verdict::OnException { branch } => {
                                format!("exception[{branch}]")
                            }
                            other => format!("{other:?}").chars().take(40).collect(),
                        };
                        *boundary.entry(why).or_default() += 1;
                    }
                }
                out.push(CandidateAnswer {
                    record_sha256: sha256_hex(line.as_bytes()),
                    line: line.to_string(),
                    measures: m,
                    served,
                    boundary,
                });
            }
            _ => {}
        }
    }
    Ok(Ok(out))
}

/// Does a univariate polynomial in d (engine text, e.g. `-13+4*d`) vanish at `d0`? (exact, core polynomial reader)
fn vanishes_at(text: &str, d0: i64) -> Option<bool> {
    use rustred_campaign_core::poly::{BigInt, Poly};
    let p = Poly::parse(text, 1, true).ok()?;
    let mut acc = BigInt::from_i128(0);
    for (j, c) in p.d_coefficients() {
        let mut pw = BigInt::from_i128(1);
        for _ in 0..j {
            pw = pw.mul(&BigInt::from_i128(d0 as i128));
        }
        for (_, cc) in c.exact_terms() {
            acc = acc.add(&cc.mul(&pw));
        }
    }
    Some(acc.is_zero())
}

/// `rustred.relation-entry.v1` entries (plan/vk5l/interfaces/relation-entry.md) for the certified recipe relations of a
/// node: body = the engine's relation record from the job's verified results stream (volatile keys dropped, canonical
/// JSON), entry_digest = sha256(canonical(body)); evidence per-key (the verb's exact weights); diagnostics: whether some
/// d-denominator factor vanishes at d = 4 (`d_poles`; the multiplicity is not in the record: `pole_order_at` null).
fn relation_entries(
    st: &CampaignState,
    node: &str,
    node_identity: &str,
    certified: &BTreeSet<(String, Vec<i32>)>,
    cdir: &Path,
) -> (Vec<String>, Value) {
    let mut out = vec![];
    let (mut with_pole, mut regular, mut unread) = (0u64, 0u64, 0u64);
    let mut files: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for (id, r) in st.inputs.iter() {
        if r.role != super::ids::ROLE_ANSWERS {
            continue;
        }
        let item = id
            .strip_prefix("closeloop:")
            .and_then(|x| x.split_once(":answers:"))
            .map(|(it, _)| it.to_string())
            .unwrap_or_default();
        if st.items.get(&item).is_none_or(|it| it.node != node) {
            continue;
        }
        let Ok(b) = read_input(st, id) else {
            unread += 1;
            continue;
        };
        for l in String::from_utf8_lossy(&b).lines() {
            let Ok(a) = serde_json::from_str::<Value>(l) else {
                continue;
            };
            if a["verdict"] != "SOLVED" {
                continue;
            }
            let key: Vec<i32> = serde_json::from_value(a["key"].clone()).unwrap_or_default();
            let src = a["source"].as_str().unwrap_or("").to_string();
            if !certified.contains(&(src.clone(), key.clone())) {
                continue;
            }
            let rp = a["results"]["path"].as_str().unwrap_or("").to_string();
            let recs = match files.get(&rp) {
                Some(v) => v.clone(),
                None => {
                    let p = cdir.join(&rp);
                    let v: Vec<Value> = match std::fs::read(&p) {
                        Ok(b)
                            if blake3_hex(&b) == a["results"]["blake3"].as_str().unwrap_or("") =>
                        {
                            String::from_utf8_lossy(&b)
                                .lines()
                                .filter_map(|x| serde_json::from_str::<Value>(x).ok())
                                .collect()
                        }
                        _ => vec![],
                    };
                    files.insert(rp.clone(), v.clone());
                    v
                }
            };
            let Some(rec) = recs.iter().find(|r| {
                r["type"] == "relation"
                    && r["ordinal"] == a["results"]["ordinal"]
                    && r["key"] == a["key"]
            }) else {
                unread += 1;
                continue;
            };
            let mut body = rec.clone();
            if let Some(o) = body.as_object_mut() {
                for k in ["seconds", "vmhwm_kb", "ordinal", "tag"] {
                    o.remove(k);
                }
            }
            let canon = body.to_string();
            let entry_digest = sha256_hex(canon.as_bytes());
            let pole4 = body["d_poles"].as_array().map(|ps| {
                ps.iter()
                    .filter_map(|p| p.as_str())
                    .any(|p| vanishes_at(p, 4) == Some(true))
            });
            match pole4 {
                Some(true) => with_pole += 1,
                Some(false) => regular += 1,
                None => {}
            }
            let e = json!({"schema": "rustred.relation-entry.v1", "node": node, "node_identity": node_identity,
                "key_frame": "table", "key": key, "entry_digest": entry_digest, "body": body,
                "recipe": {"id": src, "file_sha256": a["recipe_sha256"], "engine_digest": a["recipe_digest"]},
                "bound_to": {"engine": {"git_commit": Value::Null, "binary_sha256": a["binary_sha256"]},
                    "relation_rule": body["relation_rule"], "determinism_id": body["determinism_id"],
                    "node_identity": node_identity, "package_revision": Value::Null,
                    "symbolica_save_format": Value::Null},
                "evidence": {"level": "per-key", "proof": {"tool": "rustred.recipe-solve/exact-weights",
                    "tool_digest": a["binary_sha256"], "record_sha256": entry_digest, "verdict": "SOLVED"},
                    "fact": Value::Null},
                "diagnostics": {"pole_order_at": {"4": Value::Null}, "pole_at_4": pole4,
                    "regular_alternative": "not_tried", "recipe": src, "outside_certified_set": false},
                "written": {"by": "rustred-campaign repairflow campaign-export"}});
            out.push(e.to_string());
        }
    }
    (
        out,
        json!({"relations_with_pole_at_d4": with_pole, "relations_regular_at_d4": regular, "unread": unread}),
    )
}
