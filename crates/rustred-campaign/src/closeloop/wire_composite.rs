//! Lane wireflow (a) + (b), interfaces/repairflow.md 20: the repair flow's COMPOSITE step as CHILD JOBS (policy
//! `route0.composite_jobs`; default off = the pinned `route0.composite_input` path exactly as before).
//!
//! Per node, before P1, ROUNDS k = 1, 2, ...:
//! 1. PLAN (pass thread): the import check of the node's spec + the composites of rounds < k; its UNCOVERED keys not
//!    asked in an earlier round, grouped by face -> input `repairflow:<n>:composite:plan:r<k>` + item
//!    `<n>.rf.c<k>` (kind `symmetry-composite`); no new key (or `max_rounds` reached) -> input
//!    `repairflow:<n>:composite:done` and P1 starts with every round's relations in its spec.
//! 2. JOB: ONE `symmetry-composite` job (adapter `adapters::symcomp`, launched by the supervisor's own launch path) over
//!    the round's keys; its settled attempt is recorded as `...:settled:r<k>` (classification + the sealed results'
//!    canonical lines), the item gets `produce`.
//! 3. ROUTE (thread): the same-backend filter (`symcomp::route_settled`: image re-derived, cited first matches, mod-p
//!    composition) -> the accepted `supplied_relations` table + verification (content-addressed records); with the
//!    kernel configured (`composite_jobs.kernel`, closeloop.md 24.9) every accepted relation becomes ONE
//!    `proof-symrows` job (symcert's adapter files, item `<n>.rf.c<k>.k<j>`). Two-row forms (wireflow (b)) are
//!    kernel-only candidates. Input `...:routed:r<k>`.
//! 4. ROUND (thread, after every kernel item of the round is final): the sealed certificate streams re-verified from
//!    their run directories (`symcert::kernel_certificate_files`) -> input `repairflow:<n>:composite:r<k>` naming the
//!    round's `CompositeInput` (table, verification, kernel certificates). A `kernel_refused` certificate is a FINDING
//!    (`findings.jsonl`), never resolved by choosing a side.
//!
//! EVIDENCE: none of this is evidence by itself. Every relation enters the node's import as a `supplied_relations`
//! source and is RE-VERIFIED AT LOAD by the import (interfaces/import.md 21): kernel certificate -> per-key; same
//! backend -> imported; two-row without certificate -> not served. The closing loop's accept / descent / acyclicity
//! checks apply to these keys exactly as to any served key.
//!
//! Resume: every step is a pure function of ledger inputs and the files they pin; a restart recomputes the plan of an
//! unfinished round and compares it with the recorded one (U-1).
//!
//! Generic (L8): no node, arity, count or path is known here; programs, caps and rounds are policy data.

use super::config::NodeCfg;
use super::ids::input_import_spec;
use crate::adapters::symcomp::{self, SymCompJobConfig, SymCompRequest, VerifyOptions};
use crate::adapters::symrows::SymrowsJobRequest;
use crate::ledger::{Event, ItemEvent};
use crate::state::{CampaignState, ItemPhase, Rung};
use crate::supervisor::worksource::{AcceptedRecord, JobRequest, Routing, Settled};
use rustred_campaign_contract::classify::Classification;
use rustred_campaign_contract::fake::{FakeOp, FakeScript, OnSigterm, RssPlan};
use rustred_campaign_contract::spec::{Budgets, InputRef, JobKind};
use rustred_campaign_core::import::check::{CheckOptions, RowStatus};
use rustred_campaign_core::import::supplied_verify::CertificateSpec;
use rustred_campaign_core::import::{Evidence, TableSetSpec};
use rustred_campaign_core::repairflow::policy::{CompositeInput, PinnedFile, RepairPolicy};
use rustred_campaign_core::repairflow::{self as rf};
use rustred_campaign_core::symcert::{KernelCaps, SymRelation, SymRow, ZeroSectors, producers};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::thread::JoinHandle;

pub const KIND_COMPOSITE: &str = "symmetry-composite";
pub const KIND_KERNEL: &str = "proof-symrows";
pub const ROLE_PLAN: &str = "repairflow_composite_plan";
pub const ROLE_SETTLED: &str = "repairflow_composite_settled";
pub const ROLE_ROUTED: &str = "repairflow_composite_routed";
pub const ROLE_ROUND: &str = "repairflow_composite_round";
pub const ROLE_DONE: &str = "repairflow_composite_done";
pub const PLAN_SCHEMA: &str = "rustred.campaign.repairflow-composite-plan.v1";
pub const SETTLED_SCHEMA: &str = "rustred.campaign.repairflow-composite-settled.v1";
pub const ROUTED_SCHEMA: &str = "rustred.campaign.repairflow-composite-routed.v1";
pub const ROUND_SCHEMA: &str = "rustred.campaign.repairflow-composite-round.v1";
pub const DONE_SCHEMA: &str = "rustred.campaign.repairflow-composite-done.v1";

pub fn input_plan(n: &str, k: u32) -> String {
    format!("repairflow:{n}:composite:plan:r{k}")
}
pub fn input_settled(n: &str, k: u32) -> String {
    format!("repairflow:{n}:composite:settled:r{k}")
}
pub fn input_routed(n: &str, k: u32) -> String {
    format!("repairflow:{n}:composite:routed:r{k}")
}
pub fn input_round(n: &str, k: u32) -> String {
    format!("repairflow:{n}:composite:r{k}")
}
pub fn input_done(n: &str) -> String {
    format!("repairflow:{n}:composite:done")
}
pub fn c_item(n: &str, k: u32) -> String {
    format!("{n}.rf.c{k}")
}
pub fn k_item(n: &str, k: u32, j: usize) -> String {
    format!("{n}.rf.c{k}.k{j}")
}

fn blake3_hex(b: &[u8]) -> String {
    blake3::hash(b).to_hex().to_string()
}

fn sha256_hex(b: &[u8]) -> String {
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(b);
    h.finish_hex()
}

fn json_bytes(v: &Value) -> Vec<u8> {
    let mut b = serde_json::to_vec_pretty(v).expect("json");
    b.push(b'\n');
    b
}

/// Read a registered input, verified by its recorded BLAKE3.
pub fn read_input(st: &CampaignState, id: &str) -> Result<Vec<u8>, String> {
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

/// The `CompositeInput`s of every finished round of node `n`, in round order (round records without accepted relations
/// contribute nothing).
pub fn round_inputs(st: &CampaignState, n: &str) -> Result<Vec<(u32, CompositeInput)>, String> {
    let mut out = vec![];
    let mut k = 1;
    while st.inputs.contains_key(&input_round(n, k)) {
        let v: Value = serde_json::from_slice(&read_input(st, &input_round(n, k))?)
            .map_err(|e| format!("composite round {k}: {e}"))?;
        if !v["composite_input"].is_null() {
            let ci: CompositeInput = serde_json::from_value(v["composite_input"].clone())
                .map_err(|e| format!("composite round {k}: {e}"))?;
            out.push((k, ci));
        }
        k += 1;
    }
    Ok(out)
}

/// The spec text with every round's relations as `supplied_relations` sources `rf:symmetry-composite:r<k>` (before the
/// default recipe, after the pinned composite of `route0.composite_input`, in round order). The import RE-VERIFIES every
/// relation at load (import.md 21). `family` = the policy's group family (`route0.family`), if any.
pub fn with_round_composites(
    text: &str,
    rounds: &[(u32, CompositeInput)],
    family: Option<&str>,
) -> Result<String, String> {
    if rounds.is_empty() {
        return Ok(text.to_string());
    }
    let mut v: Value = serde_json::from_str(text).map_err(|e| format!("import spec: {e}"))?;
    let srcs = v
        .get_mut("sources")
        .and_then(Value::as_array_mut)
        .ok_or("import spec: no sources")?;
    let mut at = srcs
        .iter()
        .position(|s| s["id"] == "default-recipe")
        .unwrap_or(srcs.len());
    for (k, ci) in rounds {
        let id = format!("rf:symmetry-composite:r{k}");
        if srcs.iter().any(|s| s["id"] == id.as_str()) {
            continue;
        }
        let mut src = json!({"id": id, "kind": "supplied_relations", "class": "symmetry-composite",
            "path": ci.table.path, "sha256": ci.table.sha256,
            "verification": {"path": ci.verification.path, "sha256": ci.verification.sha256}});
        // the import's own group re-derivation of every PLAIN relation (route (a), per-key): the policy's family, else
        // the frame header's (`{}`), as route 0b's source; composites fall through to their certificate / verification
        src["verify"] = match family {
            Some(f) => json!({"family": {"path": f}}),
            None => json!({}),
        };
        if let Some(kc) = &ci.kernel_certificates {
            src["kernel_certificates"] = serde_json::to_value(kc).map_err(|e| e.to_string())?;
        }
        srcs.insert(at, src);
        at += 1;
    }
    Ok(serde_json::to_string_pretty(&v).expect("json") + "\n")
}

/// Test-only fake scripts of the composite / kernel jobs (policy `fake.composite_ops` / `fake.kernel_ops`).
#[derive(Clone, Debug, Default)]
pub struct FakeOps {
    pub composite: Option<Vec<FakeOp>>,
    pub kernel: Option<Vec<FakeOp>>,
}

pub fn script(ops: &[FakeOp]) -> FakeScript {
    FakeScript {
        script: ops.to_vec(),
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

/// A round's plan (the record of step 1).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoundPlan {
    pub schema: String,
    pub node: String,
    pub round: u32,
    pub spec_digest: String,
    pub closure_keys: u64,
    pub uncovered: u64,
    pub asked_before: u64,
    /// (face label, keys) of the NEW uncovered keys.
    pub faces: Vec<(String, Vec<Vec<i64>>)>,
    /// Keys outside the node's chart (not asked).
    pub outside: u64,
    pub keys: u64,
}

/// What the pass thread of a round plan returns: the plan + the spec text it checked (the job's import spec).
struct PlanOut {
    plan: RoundPlan,
    spec_text: String,
}

/// One kernel job of a round (its files are records under the round directory).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KernelJob {
    pub j: usize,
    pub id: String,
    pub key: Vec<i64>,
    pub producer: String,
    pub kernel_only: bool,
    /// CDIR-relative directory of the job's four input files.
    pub dir: String,
    /// BLAKE3 of request.json, relation.table.jsonl, keys.txt, symrows.jsonl.
    pub blake3: Vec<String>,
}

struct RoutedOut {
    record: Vec<u8>,
    accepts: Vec<AcceptedRecord>,
    kernel_jobs: Vec<KernelJob>,
}

/// A pass thread of round k.
type Pass<T> = (u32, JoinHandle<Result<T, String>>);

/// The per-node runtime state of the composite stage (threads only; everything else is in the ledger).
#[derive(Default)]
pub struct Stage {
    plan_pass: Option<Pass<PlanOut>>,
    route_pass: Option<Pass<RoutedOut>>,
    round_pass: Option<Pass<Vec<u8>>>,
    prog_sha: BTreeMap<String, String>,
}

/// What the stage needs from the repair flow (borrowed per call).
pub struct Ctx<'a> {
    pub cdir: &'a Path,
    pub n: &'a str,
    pub cfg: &'a NodeCfg,
    pub policy: &'a RepairPolicy,
    pub programs: &'a BTreeMap<String, PathBuf>,
    pub fake: &'a FakeOps,
    /// The node's import spec text BEFORE the round composites (with_route0 applied by the caller).
    pub base_text: Option<&'a str>,
}

fn round_dir(n: &str, k: u32) -> String {
    format!("{}/repairflow/composite/r{k}", super::ids::node_dir(n))
}

fn rec(path: String, bytes: Vec<u8>) -> AcceptedRecord {
    AcceptedRecord { path, bytes }
}

fn reg(cdir: &Path, rt: &mut Routing, id: String, role: &str, r: AcceptedRecord) {
    let f = r.file_ref();
    rt.events.push(Event::InputRegistered {
        input: id,
        role: role.into(),
        path: cdir.join(&r.path).display().to_string(),
        bytes: f.bytes,
        blake3: f.blake3,
    });
    rt.accepts.push(r);
}

fn item_final(p: &ItemPhase) -> bool {
    !matches!(
        p,
        ItemPhase::Open | ItemPhase::InProgress | ItemPhase::Attention(_)
    )
}

/// The current round of node `n`: the first k without a round record.
fn current_round(st: &CampaignState, n: &str) -> u32 {
    let mut k = 1;
    while st.inputs.contains_key(&input_round(n, k)) {
        k += 1;
    }
    k
}

/// Every key asked in rounds < k (from their plan records).
fn asked_before(st: &CampaignState, n: &str, k: u32) -> Result<BTreeSet<Vec<i32>>, String> {
    let mut s = BTreeSet::new();
    for r in 1..k {
        let p: RoundPlan = serde_json::from_slice(&read_input(st, &input_plan(n, r))?)
            .map_err(|e| format!("composite plan r{r}: {e}"))?;
        for (_, ks) in p.faces {
            for q in ks {
                s.insert(q.into_iter().map(|x| x as i32).collect());
            }
        }
    }
    Ok(s)
}

/// Step 1 (thread): load + check, the new uncovered keys by face.
#[allow(clippy::too_many_arguments)]
fn plan_pass(
    node: &str,
    round: u32,
    text: String,
    base: PathBuf,
    threads: usize,
    rows: PathBuf,
    asked: BTreeSet<Vec<i32>>,
    face_order: String,
) -> Result<PlanOut, String> {
    let spec = TableSetSpec::from_json(&text).map_err(|e| format!("import spec: {e}"))?;
    let set = rustred_campaign_core::import::load(&spec, &base)
        .map_err(|e| format!("import load: {e}"))?;
    let ev = Evidence::from_spec(&set, &base).map_err(|e| format!("evidence: {e}"))?;
    if let Some(d) = rows.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    let res = rustred_campaign_core::import::check::check_streamed(
        &set,
        &ev,
        &CheckOptions {
            threads,
            tier: None,
            max_rounds: None,
        },
        &rows,
    )
    .map_err(|e| format!("import check: {e}"))?;
    let _ = std::fs::remove_file(&rows);
    let mut uncovered = 0u64;
    let mut fresh: Vec<Vec<i32>> = vec![];
    for r in &res.rows {
        if r.status == RowStatus::Open && r.open_status.as_deref() == Some("uncovered") {
            uncovered += 1;
            if !asked.contains(&r.key) {
                fresh.push(r.key.clone());
            }
        }
    }
    let lines = set.frame.mask().to_vec();
    let (faces, outside) = rf::group_open_keys(&fresh, &lines, &face_order);
    let faces: Vec<(String, Vec<Vec<i64>>)> = faces
        .into_iter()
        .map(|f| {
            (
                f.label,
                f.open_keys
                    .into_iter()
                    .map(|k| k.into_iter().map(i64::from).collect())
                    .collect(),
            )
        })
        .collect();
    let keys = faces.iter().map(|(_, k)| k.len() as u64).sum();
    Ok(PlanOut {
        plan: RoundPlan {
            schema: PLAN_SCHEMA.into(),
            node: node.into(),
            round,
            spec_digest: res.spec_digest.clone(),
            closure_keys: res.summary.keys_total,
            uncovered,
            asked_before: asked.len() as u64,
            faces,
            outside: outside.len() as u64,
            keys,
        },
        spec_text: text,
    })
}

/// The kernel route of this configuration (closeloop.md 24.9): the configured program's file must have the stated
/// SHA-256 (no process is started).
fn kernel_route(c: &Ctx) -> Result<Option<(String, String)>, String> {
    let Some(k) = &c.policy.route0.composite_jobs.kernel else {
        return Ok(None);
    };
    let path = c
        .programs
        .get(&k.program)
        .ok_or_else(|| format!("composite kernel program {} not in the spec", k.program))?;
    let prog = crate::adapters::job::Program {
        name: k.program.clone(),
        path: path.clone(),
        blake3: String::new(),
        sha256: Some(k.sha256.clone()),
        invocation: crate::adapters::job::Invocation::Legacy,
        features_required: vec![],
    };
    match symcomp::composite_second_route_configured(Some(&prog)) {
        symcomp::CompositeRoute::Kernel { sha256, .. } => Ok(Some((k.program.clone(), sha256))),
        symcomp::CompositeRoute::SameBackend { why } => Err(why),
    }
}

/// The node's effective policy as a policy FILE (the tool's pinned `policy` input).
fn policy_file_bytes(p: &RepairPolicy) -> Vec<u8> {
    json_bytes(&json!({"schema": rf::POLICY_SCHEMA, "repair_policy": p, "nodes": {}}))
}

impl Stage {
    fn program_sha(
        &mut self,
        programs: &BTreeMap<String, PathBuf>,
        name: &str,
    ) -> Result<String, String> {
        if let Some(s) = self.prog_sha.get(name) {
            return Ok(s.clone());
        }
        let p = programs
            .get(name)
            .ok_or_else(|| format!("program {name} not in the spec"))?;
        let b = std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?;
        let s = sha256_hex(&b);
        self.prog_sha.insert(name.into(), s.clone());
        Ok(s)
    }

    /// POLL: advance node `n`'s composite stage by what the ledger says. Returns true when the stage is done (P1 may
    /// start).
    pub fn poll(&mut self, st: &CampaignState, c: &Ctx, rt: &mut Routing) -> Result<bool, String> {
        let n = c.n;
        if st.inputs.contains_key(&input_done(n)) {
            return Ok(true);
        }
        if !st.inputs.contains_key(&input_import_spec(n)) {
            return Ok(false);
        }
        let k = current_round(st, n);
        let node = c.cfg.node.clone();
        // step 1: the plan of round k
        if !st.inputs.contains_key(&input_plan(n, k)) {
            let Some(base_text) = c.base_text else {
                return Ok(false);
            };
            if self.plan_pass.as_ref().is_some_and(|(r, _)| *r != k) {
                self.plan_pass = None;
            }
            if self.plan_pass.is_none() {
                let rounds = round_inputs(st, n)?;
                let text =
                    with_round_composites(base_text, &rounds, c.policy.route0.family.as_deref())?;
                let asked = asked_before(st, n, k)?;
                let rows = c.cdir.join(format!("{}/check.rows.jsonl", round_dir(n, k)));
                let (node2, base, threads) =
                    (node.clone(), c.cfg.base(), c.cfg.check_threads.max(1));
                let fo = c.policy.symbolic.face_order.clone();
                let h = std::thread::Builder::new()
                    .name(format!("repairflow-composite-plan-{n}-r{k}"))
                    .spawn(move || plan_pass(&node2, k, text, base, threads, rows, asked, fo))
                    .map_err(|e| format!("composite plan thread: {e}"))?;
                self.plan_pass = Some((k, h));
            }
            if !self
                .plan_pass
                .as_ref()
                .is_some_and(|(_, h)| h.is_finished())
            {
                return Ok(false);
            }
            let (_, h) = self.plan_pass.take().expect("plan pass");
            let out = h
                .join()
                .map_err(|_| "composite plan thread panicked".to_string())??;
            let dir = round_dir(n, k);
            let pb = json_bytes(&serde_json::to_value(&out.plan).map_err(|e| e.to_string())?);
            let cj = &c.policy.route0.composite_jobs;
            if out.plan.keys == 0 || k > cj.max_rounds {
                let stop = if out.plan.keys == 0 {
                    "no_new_key"
                } else {
                    "max_rounds"
                };
                let d = json!({"schema": DONE_SCHEMA, "node": node, "rounds": k - 1, "stop": stop,
                    "left_uncovered": out.plan.uncovered, "left_new": out.plan.keys, "plan": out.plan});
                reg(
                    c.cdir,
                    rt,
                    input_done(n),
                    ROLE_DONE,
                    rec(
                        format!("{}/repairflow/composite/done.json", super::ids::node_dir(n)),
                        json_bytes(&d),
                    ),
                );
                return Ok(false);
            }
            // the job's files: the round's import spec + the effective policy (content-addressed) + keys / request
            rt.accepts.push(rec(
                format!("{dir}/import-spec.json"),
                out.spec_text.into_bytes(),
            ));
            reg(
                c.cdir,
                rt,
                input_plan(n, k),
                ROLE_PLAN,
                rec(format!("{dir}/plan.json"), pb),
            );
            let item = c_item(n, k);
            if !st.items.contains_key(&item) {
                rt.events.push(Event::ItemCreated {
                    item,
                    node: node.clone(),
                    kind: KIND_COMPOSITE.into(),
                    rung: Rung(0),
                    parent: None,
                    inputs: vec![input_plan(n, k)],
                    label: format!("composite r{k}"),
                    weight: out.plan.keys,
                });
            }
            return Ok(false);
        }
        // step 3: the settled composite job -> routed (thread)
        if st.inputs.contains_key(&input_settled(n, k))
            && !st.inputs.contains_key(&input_routed(n, k))
        {
            if self.route_pass.as_ref().is_some_and(|(r, _)| *r != k) {
                self.route_pass = None;
            }
            if self.route_pass.is_none() {
                let settled: SettledRec =
                    serde_json::from_slice(&read_input(st, &input_settled(n, k))?)
                        .map_err(|e| format!("composite settled r{k}: {e}"))?;
                let kroute = kernel_route(c);
                let kcfg = c.policy.route0.composite_jobs.kernel.clone();
                let pieces = c.policy.pieces.clone();
                let cdir = c.cdir.to_path_buf();
                let (n2, node2) = (n.to_string(), node.clone());
                let h = std::thread::Builder::new()
                    .name(format!("repairflow-composite-route-{n}-r{k}"))
                    .spawn(move || route_pass(&cdir, &n2, &node2, k, settled, kroute, kcfg, pieces))
                    .map_err(|e| format!("composite route thread: {e}"))?;
                self.route_pass = Some((k, h));
            }
            if !self
                .route_pass
                .as_ref()
                .is_some_and(|(_, h)| h.is_finished())
            {
                return Ok(false);
            }
            let (_, h) = self.route_pass.take().expect("route pass");
            let out = h
                .join()
                .map_err(|_| "composite route thread panicked".to_string())??;
            rt.accepts.extend(out.accepts);
            let dir = round_dir(n, k);
            reg(
                c.cdir,
                rt,
                input_routed(n, k),
                ROLE_ROUTED,
                rec(format!("{dir}/routed.json"), out.record),
            );
            for kj in &out.kernel_jobs {
                let item = k_item(n, k, kj.j);
                if !st.items.contains_key(&item) {
                    rt.events.push(Event::ItemCreated {
                        item,
                        node: node.clone(),
                        kind: KIND_KERNEL.into(),
                        rung: Rung(0),
                        parent: None,
                        inputs: vec![input_routed(n, k)],
                        label: kj.id.clone(),
                        weight: 1,
                    });
                }
            }
            return Ok(false);
        }
        // a composite item that ended without a sealed result and was skipped by the operator: an empty round
        let citem = c_item(n, k);
        if !st.inputs.contains_key(&input_settled(n, k))
            && st
                .items
                .get(&citem)
                .is_some_and(|it| it.phase == ItemPhase::Skipped)
        {
            let d = json!({"schema": ROUND_SCHEMA, "node": node, "round": k, "composite_input": null,
                "skipped": true});
            reg(
                c.cdir,
                rt,
                input_round(n, k),
                ROLE_ROUND,
                rec(format!("{}/round.json", round_dir(n, k)), json_bytes(&d)),
            );
            return Ok(false);
        }
        // step 4: every kernel item final -> the round record (thread)
        if st.inputs.contains_key(&input_routed(n, k)) {
            let routed: Value = serde_json::from_slice(&read_input(st, &input_routed(n, k))?)
                .map_err(|e| format!("composite routed r{k}: {e}"))?;
            let jobs: Vec<KernelJob> =
                serde_json::from_value(routed["kernel"]["jobs"].clone()).unwrap_or_default();
            let all_final = jobs.iter().all(|kj| {
                st.items
                    .get(&k_item(n, k, kj.j))
                    .is_some_and(|it| item_final(&it.phase))
            });
            if !all_final {
                return Ok(false);
            }
            if self.round_pass.as_ref().is_some_and(|(r, _)| *r != k) {
                self.round_pass = None;
            }
            if self.round_pass.is_none() {
                // the produced attempts' run directories (resume: from the ledger)
                let mut runs: Vec<(KernelJob, Option<String>)> = vec![];
                for kj in jobs {
                    let it = st.items.get(&k_item(n, k, kj.j));
                    let rd = it
                        .and_then(|it| it.produced.as_ref())
                        .and_then(|p| st.jobs.get(&p.job))
                        .and_then(|j| j.run_dir.clone());
                    runs.push((kj, rd));
                }
                let cdir = c.cdir.to_path_buf();
                let node2 = node.clone();
                let n2 = n.to_string();
                let h = std::thread::Builder::new()
                    .name(format!("repairflow-composite-round-{n}-r{k}"))
                    .spawn(move || round_pass(&cdir, &n2, &node2, k, routed, runs))
                    .map_err(|e| format!("composite round thread: {e}"))?;
                self.round_pass = Some((k, h));
            }
            if !self
                .round_pass
                .as_ref()
                .is_some_and(|(_, h)| h.is_finished())
            {
                return Ok(false);
            }
            let (_, h) = self.round_pass.take().expect("round pass");
            let b = h
                .join()
                .map_err(|_| "composite round thread panicked".to_string())??;
            let dir = round_dir(n, k);
            reg(
                c.cdir,
                rt,
                input_round(n, k),
                ROLE_ROUND,
                rec(format!("{dir}/round.json"), b),
            );
        }
        Ok(false)
    }

    /// READY: the job requests of the stage's open items (composite + kernel).
    pub fn ready(&mut self, st: &CampaignState, c: &Ctx) -> Vec<JobRequest> {
        let mut out = vec![];
        let n = c.n;
        let node = &c.cfg.node;
        for (item, it) in st.items.iter().filter(|(_, it)| {
            it.node == *node && (it.kind == KIND_COMPOSITE || it.kind == KIND_KERNEL)
        }) {
            let job = match it.phase {
                ItemPhase::Open => Some(format!("{item}-t{}", it.jobs.len())),
                ItemPhase::InProgress => {
                    match it.jobs.last().and_then(|l| st.jobs.get(l)).map(|r| r.state) {
                        Some(crate::state::AttemptState::Queued) => it.jobs.last().cloned(),
                        _ => None,
                    }
                }
                _ => None,
            };
            let Some(job) = job else { continue };
            let r = if it.kind == KIND_COMPOSITE {
                self.composite_request(st, c, item, &job)
            } else {
                self.kernel_request(st, c, item, &job)
            };
            match r {
                Ok(r) => out.push(r),
                Err(e) => eprintln!("repairflow {n}: no job for {item}: {e}"),
            }
        }
        out
    }

    fn composite_request(
        &mut self,
        st: &CampaignState,
        c: &Ctx,
        item: &str,
        job: &str,
    ) -> Result<JobRequest, String> {
        let n = c.n;
        let k: u32 = item
            .rsplit(".rf.c")
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or("composite item id")?;
        let plan: RoundPlan = serde_json::from_slice(&read_input(st, &input_plan(n, k))?)
            .map_err(|e| format!("composite plan r{k}: {e}"))?;
        let cj = &c.policy.route0.composite_jobs;
        let dir = c.cdir.join(round_dir(n, k));
        let pol = dir.join("policy.json");
        let pb = policy_file_bytes(c.policy);
        if std::fs::read(&pol).ok().as_deref() != Some(pb.as_slice()) {
            crate::atomic::write_atomic(&pol, &pb)
                .map_err(|e| format!("{}: {e}", pol.display()))?;
        }
        let req = SymCompRequest {
            node: Some(c.cfg.node.clone()),
            import_spec: dir.join("import-spec.json"),
            import_base: c.cfg.base(),
            faces: plan.faces.clone(),
            policy: Some(pol),
            threads: cj.threads.max(1),
            limit: None,
        };
        let prog_sha = self.program_sha(c.programs, &cj.program)?;
        // content-addressed job directory: a restart reuses the files written before; the program's SHA-256 and the
        // policy are part of the address (a changed binary or policy gets fresh files: the request pins both)
        let mut addr = req.keys_bytes();
        addr.extend_from_slice(prog_sha.as_bytes());
        addr.extend_from_slice(&pb);
        let tag = &blake3_hex(&addr)[..16];
        let jdir = dir.join(format!("job.{tag}"));
        let files = if jdir.join("request.json").exists() {
            existing_job_files(&jdir)?
        } else {
            symcomp::job_files(&jdir, &req, &prog_sha).map_err(|e| e.to_string())?
        };
        let fake = c.fake.composite.as_ref().map(|o| script(o));
        let cfg = SymCompJobConfig {
            program: cj.program.clone(),
            cap_bytes: cj.cap_bytes,
            expected_peak_bytes: cj.expected_peak_bytes,
            cores: cj.threads.max(1) as usize,
            heartbeat_s: cj.heartbeat_s(),
            priority: 2_000_000_000,
            fake,
        };
        Ok(symcomp::job_request(
            item,
            job,
            &files,
            &cfg,
            vec![input_plan(n, k)],
        ))
    }

    fn kernel_request(
        &mut self,
        st: &CampaignState,
        c: &Ctx,
        item: &str,
        job: &str,
    ) -> Result<JobRequest, String> {
        let n = c.n;
        let it = st.items.get(item).ok_or("kernel item")?;
        let routed_id = it
            .inputs
            .first()
            .cloned()
            .ok_or("kernel item without input")?;
        let routed: Value = serde_json::from_slice(&read_input(st, &routed_id)?)
            .map_err(|e| format!("{routed_id}: {e}"))?;
        let jobs: Vec<KernelJob> =
            serde_json::from_value(routed["kernel"]["jobs"].clone()).map_err(|e| e.to_string())?;
        let j: usize = item
            .rsplit(".k")
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or("kernel item id")?;
        let kj = jobs
            .iter()
            .find(|x| x.j == j)
            .ok_or("kernel job not in the routed record")?;
        let k = c
            .policy
            .route0
            .composite_jobs
            .kernel
            .as_ref()
            .ok_or("no kernel configured")?;
        let names = [
            "request.json",
            "relation.table.jsonl",
            "keys.txt",
            "symrows.jsonl",
        ];
        let roles = ["request", "table", "keys", "symrows"];
        let mut inputs = vec![];
        for ((f, role), b3) in names.iter().zip(roles).zip(&kj.blake3) {
            let p = c.cdir.join(&kj.dir).join(f);
            inputs.push(InputRef {
                role: role.into(),
                path: p.display().to_string(),
                blake3: b3.clone(),
                upto: None,
            });
        }
        let _ = n;
        Ok(JobRequest {
            job: job.into(),
            item: item.into(),
            rung: Rung(0),
            kind: JobKind::ProofSymrows,
            priority: 1_900_000_000,
            cap_bytes: k.cap_bytes,
            expected_peak_bytes: k.expected_peak_bytes,
            cores: 1,
            heartbeat_s: k.heartbeat_s(),
            stop_grace_s: 5,
            program: k.program.clone(),
            args: vec![],
            inputs,
            depends_on: vec![routed_id],
            budgets: Budgets::default(),
            sample_seed: 0,
            fake: c.fake.kernel.as_ref().map(|o| script(o)),
        })
    }

    /// ROUTE: a settled attempt of a stage item.
    pub fn route(
        &mut self,
        st: &CampaignState,
        c: &Ctx,
        done: &Settled,
    ) -> Result<Routing, String> {
        let mut rt = Routing::default();
        let it = st.items.get(&done.item).ok_or("unknown item")?;
        let cl = &done.classification;
        if it.kind == KIND_COMPOSITE {
            let k: u32 = done
                .item
                .rsplit(".rf.c")
                .next()
                .and_then(|s| s.parse().ok())
                .ok_or("composite item id")?;
            match &done.table {
                Some(t) if !cl.attention => {
                    let s = SettledRec {
                        schema: SETTLED_SCHEMA.into(),
                        job: done.job.clone(),
                        attempt: done.attempt,
                        classification: cl.clone(),
                        table: t.clone(),
                        canonical_lines: done.canonical_lines.clone(),
                    };
                    let b = json_bytes(&serde_json::to_value(&s).map_err(|e| e.to_string())?);
                    if !st.inputs.contains_key(&input_settled(c.n, k)) {
                        reg(
                            c.cdir,
                            &mut rt,
                            input_settled(c.n, k),
                            ROLE_SETTLED,
                            rec(format!("{}/settled.json", round_dir(c.n, k)), b),
                        );
                    }
                    // a sealed result that ships relations -> produce; one that ships none (row 35: per-key outcomes
                    // only, the row accepts nothing) -> the item ends (escalated: no further rung exists), the
                    // round goes on from the settled record
                    if cl.accept == rustred_campaign_contract::classify::Accept::SuppliedRelations {
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
                                reason: format!(
                                    "repairflow: the composite round shipped no relation (row {}{}: {})",
                                    cl.row, cl.sub, cl.why
                                ),
                            },
                        });
                    }
                }
                _ => {
                    let attempts = it.jobs.len() as u32;
                    if !cl.attention && attempts < c.policy.route0.composite_jobs.max_attempts {
                        rt.events.push(Event::Item {
                            item: done.item.clone(),
                            ev: ItemEvent::Requeue {
                                reason: format!(
                                    "composite job row {}{}: {}",
                                    cl.row, cl.sub, cl.why
                                ),
                            },
                        });
                    } else {
                        rt.events.push(Event::Item {
                            item: done.item.clone(),
                            ev: ItemEvent::Attention {
                                code: "repair_composite_failed".into(),
                                text: format!(
                                    "repairflow: the symmetry-composite job ended without a sealed result (row {}{}: {}): operator retry or skip (skip = an empty round)",
                                    cl.row, cl.sub, cl.why
                                ),
                            },
                        });
                    }
                }
            }
            return Ok(rt);
        }
        // kernel item: a certificate (row 40) -> produce; anything else: no lift (the relation keeps its level)
        if cl.row == 40
            && let Some(t) = &done.table
        {
            rt.events.push(Event::Item {
                item: done.item.clone(),
                ev: ItemEvent::Produce {
                    job: done.job.clone(),
                    attempt: done.attempt,
                    table: t.clone(),
                },
            });
        } else if cl.attention && cl.row != 41 && cl.row != 42 {
            rt.events.push(Event::Item {
                item: done.item.clone(),
                ev: ItemEvent::Attention {
                    code: "repair_kernel_attention".into(),
                    text: format!(
                        "repairflow: proof-symrows row {}{}: {}",
                        cl.row, cl.sub, cl.why
                    ),
                },
            });
        } else {
            rt.events.push(Event::Item {
                item: done.item.clone(),
                ev: ItemEvent::Escalate {
                    to: Rung(1),
                    reason: format!(
                        "repairflow: no kernel certificate (row {}{}: {}): the relation keeps its level",
                        cl.row, cl.sub, cl.why
                    ),
                },
            });
        }
        Ok(rt)
    }
}

/// The settled composite attempt as recorded (step 2).
#[derive(Clone, Debug, Serialize, Deserialize)]
struct SettledRec {
    schema: String,
    job: String,
    attempt: u32,
    classification: Classification,
    table: crate::ledger::FileRef,
    canonical_lines: Vec<String>,
}

/// The JobFiles of a job directory written before (a restart): the request names its pinned inputs.
fn existing_job_files(dir: &Path) -> Result<symcomp::JobFiles, String> {
    let rp = dir.join("request.json");
    let rb = std::fs::read(&rp).map_err(|e| format!("{}: {e}", rp.display()))?;
    let r: Value = serde_json::from_slice(&rb).map_err(|e| format!("{}: {e}", rp.display()))?;
    let mut inputs = vec![InputRef {
        role: "request".into(),
        path: rp.display().to_string(),
        blake3: blake3_hex(&rb),
        upto: None,
    }];
    for role in ["import_spec", "keys", "policy"] {
        let Some(p) = r[role]["path"].as_str() else {
            continue;
        };
        let b = std::fs::read(p).map_err(|e| format!("{p}: {e}"))?;
        if r[role]["sha256"].as_str() != Some(sha256_hex(&b).as_str()) {
            return Err(format!("{p}: changed since the job files were written"));
        }
        inputs.push(InputRef {
            role: role.into(),
            path: p.into(),
            blake3: blake3_hex(&b),
            upto: None,
        });
    }
    Ok(symcomp::JobFiles {
        request: rp,
        keys: dir.join("keys.jsonl"),
        inputs,
    })
}

/// Step 3 (thread): the same-backend filter of the settled attempt; the accepted records; the kernel jobs' files.
#[allow(clippy::too_many_arguments)]
fn route_pass(
    cdir: &Path,
    n: &str,
    node: &str,
    k: u32,
    s: SettledRec,
    kroute: Result<Option<(String, String)>, String>,
    kcfg: Option<rustred_campaign_core::repairflow::policy::KernelJobs>,
    pieces: rustred_campaign_core::repairflow::policy::PiecesStep,
) -> Result<RoutedOut, String> {
    let settled = Settled {
        job: s.job.clone(),
        attempt: s.attempt,
        item: c_item(n, k),
        rung: Rung(0),
        classification: s.classification.clone(),
        table: Some(s.table.clone()),
        canonical_lines: s.canonical_lines.clone(),
        canonical_blake3: None,
    };
    let results = cdir.join(&s.table.path);
    let run_root = results
        .parent()
        .and_then(Path::parent)
        .ok_or("results path without a run directory")?;
    let spec_b =
        std::fs::read(rustred_campaign_contract::layout::RunLayout::new(run_root).job_spec())
            .map_err(|e| format!("job.json: {e}"))?;
    let spec: rustred_campaign_contract::spec::JobSpec =
        serde_json::from_slice(&spec_b).map_err(|e| format!("job.json: {e}"))?;
    let ctx = symcomp::load_import(&spec)?;
    let routed = symcomp::route_settled(cdir, &settled, Some(&ctx), &VerifyOptions::default())?
        .unwrap_or_default();
    let dir = round_dir(n, k);
    let mut accepts = vec![];
    let counts = routed.counts();
    let mut table_pin = Value::Null;
    let mut ver_pin = Value::Null;
    if !routed.verified.accepted.is_empty() {
        let recs = routed.records(&dir);
        table_pin = json!({"path": cdir.join(&recs[0].path).display().to_string(), "sha256": sha256_hex(&recs[0].bytes)});
        ver_pin = json!({"path": cdir.join(&recs[1].path).display().to_string(), "sha256": sha256_hex(&recs[1].bytes)});
        accepts.extend(recs);
    } else {
        // the per-key outcomes are kept even when nothing was accepted
        accepts.extend(routed.records(&dir).into_iter().skip(2));
    }
    // the kernel jobs (one per accepted relation)
    let mut kernel_jobs: Vec<KernelJob> = vec![];
    let mut not_expressible: Vec<Value> = vec![];
    let (route_word, route_why, ksha) = match &kroute {
        Ok(Some((_, sha))) => ("kernel", Value::Null, Some(sha.clone())),
        Ok(None) => ("same_backend", json!("no proof kernel configured"), None),
        Err(why) => ("same_backend", json!(why), None),
    };
    if let (Some(sha), Some(kc)) = (&ksha, &kcfg)
        && !routed.verified.accepted.is_empty()
    {
        let fpath = rf::symflow::frame_table_path(&ctx.set).ok_or("frame_from: no table file")?;
        let fpath = if Path::new(&fpath).is_absolute() {
            PathBuf::from(&fpath)
        } else {
            PathBuf::from("/").join(&fpath)
        };
        let head = rustred_campaign_core::import::lines::read_header("frame", &fpath)
            .map_err(|e| format!("frame header: {e}"))?;
        let family = head.raw["family_source"]
            .as_str()
            .ok_or("frame header: no family_source")?
            .to_string();
        let census: Option<ZeroSectors> = if kc.census == "frame" {
            rustred_campaign_core::symcert::census::table_record(&fpath)?
                .map(|l| {
                    rustred_campaign_core::symcert::census::from_record_line(
                        &l,
                        &format!(
                            "engine table {} (zero_sectors record sha256 {})",
                            fpath.display(),
                            sha256_hex(l.as_bytes())
                        ),
                    )
                })
                .transpose()?
        } else {
            None
        };
        let mats: Vec<Option<Vec<i64>>> = ctx
            .group
            .proofs()
            .iter()
            .map(|p| Some(p.matrix.clone()))
            .collect();
        let caps = KernelCaps {
            arity: ctx.set.frame.arity(),
            max_rows: kc.max_rows,
            max_seeds: kc.max_seeds,
        };
        let table_txt = String::from_utf8_lossy(&routed.table).into_owned();
        let ver_txt = String::from_utf8_lossy(&routed.verification).into_owned();
        let entries = producers::composites(&table_txt, &ver_txt)?;
        // catalogue records by SHA-256 (only the cited ones), from the import's own tables
        let want: BTreeSet<String> = entries
            .iter()
            .flat_map(|e| e.replacements.iter().map(|r| r.record_sha256.clone()))
            .collect();
        let mut cat: BTreeMap<String, Value> = BTreeMap::new();
        if !want.is_empty() {
            for (t, recs) in ctx.set.rules.iter().enumerate() {
                for (r, info) in recs.iter().enumerate() {
                    if want.contains(&info.record_sha256) && !cat.contains_key(&info.record_sha256)
                    {
                        let line =
                            rustred_campaign_core::import::exact_facts::record_line(&ctx.set, t, r)
                                .map_err(|e| format!("catalogue record: {e}"))?;
                        let v: Value = serde_json::from_slice(&line).map_err(|e| e.to_string())?;
                        cat.insert(info.record_sha256.clone(), v);
                    }
                }
            }
        }
        let kernel_only: BTreeMap<Vec<i64>, bool> = routed
            .verified
            .accepted
            .iter()
            .map(|a| (a.key.clone(), a.kernel_only))
            .collect();
        for e in &entries {
            let two_row = e.verification["kind"]
                .as_str()
                .is_some_and(|w| w.starts_with("two_row"));
            // plain relations: re-derived exactly by the import from its own verified group (route (a)); the kernel
            // only on request (`kernel.plain`)
            if e.plain && !two_row && !kc.plain {
                continue;
            }
            let built = if two_row {
                two_row_relation(e, &mats, &|d| cat.get(d).cloned(), census.clone())
            } else {
                producers::composite_relation(e, &mats, &|d| cat.get(d).cloned(), census.clone())
            };
            let rel = match built.and_then(|r| r.expressible(&caps).map(|_| r)) {
                Ok(mut r) => {
                    r.id = format!("r{k}:{}", r.id);
                    r
                }
                Err(nx) => {
                    not_expressible.push(json!({"key": e.key, "class": nx.class, "why": nx.why}));
                    continue;
                }
            };
            let req = match SymrowsJobRequest::new(rel, &family, sha, kc.max_primes) {
                Ok(r) => r,
                Err(why) => {
                    not_expressible.push(json!({"key": e.key, "class": "request", "why": why}));
                    continue;
                }
            };
            let j = kernel_jobs.len();
            let kdir = format!("{dir}/kernel/{j}");
            let files: [(&str, Vec<u8>); 4] = [
                ("request.json", req.request_bytes()),
                ("relation.table.jsonl", req.files.table.clone()),
                ("keys.txt", req.files.keys.clone()),
                ("symrows.jsonl", req.files.symrows.clone()),
            ];
            let mut b3 = vec![];
            for (f, b) in files {
                b3.push(blake3_hex(&b));
                accepts.push(rec(format!("{kdir}/{f}"), b));
            }
            kernel_jobs.push(KernelJob {
                j,
                id: req.relation.id.clone(),
                key: e.key.clone(),
                producer: req.relation.producer.clone(),
                kernel_only: kernel_only.get(&e.key).copied().unwrap_or(false),
                dir: kdir,
                blake3: b3,
            });
        }
    }
    // wireflow (c), TRICKS 2: lane regions' fit (the tool's sealed `regions.json`: regions per face fitted on the per-key
    // choices) carried into the round as a RECORD: a generalisation, never a serving condition (each key is served by its
    // own verified relation); summary per face (regions, exact or not) in the routed record
    let regions_v = match sealed_child_file(run_root, &spec, &s, "regions.json") {
        Ok(b) => {
            let v: Value = serde_json::from_slice(&b).unwrap_or(Value::Null);
            accepts.push(rec(format!("{dir}/regions.json"), b));
            // wireflow (c): the keys every face's fit was made on (the sealed per-key records with status ok), for the
            // P5 shipping test "exact on the face's certified keys"
            if let Ok(cb) = sealed_child_file(run_root, &spec, &s, "choices.jsonl") {
                let mut by_face: BTreeMap<String, Vec<Value>> = BTreeMap::new();
                for l in String::from_utf8_lossy(&cb).lines() {
                    if let Ok(c) = serde_json::from_str::<Value>(l)
                        && c["status"] == "ok"
                        && let Some(f) = c["face"].as_str()
                    {
                        by_face
                            .entry(f.to_string())
                            .or_default()
                            .push(c["key"].clone());
                    }
                }
                accepts.push(rec(
                    format!("{dir}/regions.keys.json"),
                    json_bytes(&json!({"faces": by_face})),
                ));
            }
            regions_summary(&v)
        }
        Err(e) => json!({"error": e}),
    };
    // wireflow (d): lane pieces' planner over the round's keys (candidates = the tool's sealed per-key choices)
    let pieces_v = if pieces.plan {
        match plan_round(run_root, &spec, &s, &ctx, &pieces) {
            Ok((plan_b, summary)) => {
                accepts.push(rec(format!("{dir}/pieces.plan.json"), plan_b));
                summary
            }
            Err(e) => json!({"error": e}),
        }
    } else {
        Value::Null
    };
    let refused: Vec<Value> = routed
        .verified
        .refused
        .iter()
        .take(50)
        .map(|(i, k, w)| json!({"index": i, "key": k, "why": w}))
        .collect();
    let record = json!({"schema": ROUTED_SCHEMA, "node": node, "round": k, "job": s.job, "attempt": s.attempt,
        "counts": counts, "accepted": routed.verified.accepted.len(),
        "kernel_only": routed.verified.accepted.iter().filter(|a| a.kernel_only).count(),
        "refused": routed.verified.refused.len(), "refused_first": refused,
        "table": table_pin, "verification": ver_pin,
        "kernel": {"route": route_word, "why": route_why, "sha256": ksha, "jobs": kernel_jobs,
                   "not_expressible": not_expressible},
        "pieces": pieces_v, "regions": regions_v});
    Ok(RoutedOut {
        record: json_bytes(&record),
        accepts,
        kernel_jobs,
    })
}

/// A file of the child's sealed output (`out/symflow/<name>`) whose SHA-256 is the results' file record.
fn sealed_child_file(
    run_root: &Path,
    spec: &rustred_campaign_contract::spec::JobSpec,
    s: &SettledRec,
    name: &str,
) -> Result<Vec<u8>, String> {
    let lines: Vec<Vec<u8>> = s
        .canonical_lines
        .iter()
        .map(|l| l.as_bytes().to_vec())
        .collect();
    let (res, _) = rustred_campaign_contract::classify::symcomp::tally(&lines, spec)?;
    let want = res
        .files
        .get(name)
        .map(|x| x.0.clone())
        .ok_or_else(|| format!("the sealed results name no {name}"))?;
    let path = run_root
        .join("out")
        .join(rustred_campaign_contract::classify::symcomp::CHILD_DIR)
        .join(name);
    let b = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    if sha256_hex(&b) != want {
        return Err(format!("{name} changed after it was sealed"));
    }
    Ok(b)
}

/// Per face: keys, regions, residual and unservable keys, method and digest of regionfit's record.
fn regions_summary(v: &Value) -> Value {
    let mut out = serde_json::Map::new();
    let faces = v.get("faces").or(Some(v));
    if let Some(m) = faces.and_then(Value::as_object) {
        for (face, f) in m {
            if !f.is_object() {
                continue;
            }
            out.insert(
                face.clone(),
                json!({"keys": f["keys"].clone(), "regions": f["regions"].as_array().map(Vec::len),
                    "residual": f["residual"].as_array().map(Vec::len),
                    "unservable": f["unservable"].as_array().map(Vec::len),
                    "method": f["fit"]["method"].clone(), "digest": f["digest"].clone()}),
            );
        }
    }
    Value::Object(out)
}

/// wireflow (d): the pieces plan of a round: the tool's SEALED `choices.jsonl` (SHA-256 = the results' file record),
/// the round's keys as the open set, bound to the import's table set (G2). Returns (plan JSON, summary).
fn plan_round(
    run_root: &Path,
    spec: &rustred_campaign_contract::spec::JobSpec,
    s: &SettledRec,
    ctx: &symcomp::ImportCtx,
    pieces: &rustred_campaign_core::repairflow::policy::PiecesStep,
) -> Result<(Vec<u8>, Value), String> {
    use rustred_campaign_core::pieces::{import_edges, keychoices, plan};
    let b = sealed_child_file(run_root, spec, s, "choices.jsonl")?;
    let path = run_root
        .join("out")
        .join(rustred_campaign_contract::classify::symcomp::CHILD_DIR)
        .join("choices.jsonl");
    let _ = b;
    let records = keychoices::read_key_choices(&path)?;
    let open: BTreeSet<Vec<i32>> = records.iter().map(|r| r.key.clone()).collect();
    let mask = ctx.lines.clone();
    let face_of = |k: &[i32]| {
        rustred_campaign_core::worklist::items::Face::of_key(k, &mask)
            .map(|f| f.label())
            .unwrap_or_default()
    };
    let mut input = keychoices::plan_input_from_key_choices(
        ctx.set.frame.arity(),
        &records,
        &open,
        &face_of,
        &pieces.options,
    )?
    .with_table_set(import_edges::table_set_digest(&ctx.set));
    input.ranking = pieces.ranking.clone();
    let p = plan::plan(&ctx.set.cmp, &input).map_err(|e| format!("pieces plan: {e:?}"))?;
    import_edges::check_plan_table_set(&p, &ctx.set)?;
    let summary = json!({"verdict": p.face_report.verdict, "keys": p.keys, "chosen": p.chosen.len(),
        "refused": p.refused.len(), "core_open": p.core_open.len(), "piece_order": p.piece_order,
        "face_level_cyclic": p.face_report.face_level_cyclic,
        "piece_level_acyclic": p.face_report.piece_level_acyclic,
        "key_level_acyclic": p.face_report.key_level_acyclic,
        "table_set_digest": p.table_set_digest});
    Ok((
        json_bytes(&serde_json::to_value(&p).map_err(|e| e.to_string())?),
        summary,
    ))
}

/// The face -> group rank of the LAST round with a pieces plan (wireflow (d)): the face jobs of P1 are issued in this
/// order (earlier group first; faces of one cyclic piece group share a rank). Empty without a plan.
pub fn piece_rank(st: &CampaignState, n: &str) -> BTreeMap<String, usize> {
    let mut out = BTreeMap::new();
    let mut k = 1;
    while st.inputs.contains_key(&input_routed(n, k)) {
        if let Ok(b) = read_input(st, &input_routed(n, k))
            && let Ok(v) = serde_json::from_slice::<Value>(&b)
            && let Some(groups) = v["pieces"]["piece_order"].as_array()
        {
            out.clear();
            for (g, grp) in groups.iter().enumerate() {
                for p in grp.as_array().into_iter().flatten() {
                    if let Some(face) = p.as_str().and_then(|l| l.split('#').next()) {
                        out.entry(face.to_string()).or_insert(g);
                    }
                }
            }
        }
        k += 1;
    }
    out
}

/// The relation of a two-row form (wireflow (b)): the rows of its combination (element, seed) + for a
/// `two_row_composite` the catalogue rules' seeds at their replaced terms.
fn two_row_relation(
    e: &producers::CompositeEntry,
    matrices: &[Option<Vec<i64>>],
    catalogue: &dyn Fn(&str) -> Option<Value>,
    zero_sectors: Option<ZeroSectors>,
) -> Result<SymRelation, rustred_campaign_core::symcert::NotExpressible> {
    let nx = |class: &'static str, why: String| rustred_campaign_core::symcert::NotExpressible {
        class,
        why,
    };
    let mut rows = vec![];
    for r in e.verification["combination"]["rows"]
        .as_array()
        .ok_or_else(|| {
            nx(
                "two_row_without_rows",
                "verification line has no combination rows".into(),
            )
        })?
    {
        let el = r["element"]
            .as_u64()
            .ok_or_else(|| nx("two_row_without_rows", "row element".into()))?
            as usize;
        let seed: Vec<i64> = serde_json::from_value(r["seed"].clone())
            .map_err(|x| nx("two_row_without_rows", format!("row seed: {x}")))?;
        let m = matrices
            .get(el)
            .cloned()
            .flatten()
            .ok_or_else(|| nx("element_without_matrix", format!("element {el}")))?;
        rows.push(SymRow {
            seed,
            element: el,
            matrix: m,
        });
    }
    // the catalogue part: as composite_relation does (a composite of one row + the replacements)
    let mut cat = vec![];
    for r in &e.replacements {
        let rec = catalogue(&r.record_sha256).ok_or_else(|| {
            nx(
                "catalogue_record_missing",
                format!(
                    "catalogue record {}#{} (sha256 {}) not found",
                    r.source, r.label, r.record_sha256
                ),
            )
        })?;
        let seeds =
            rustred_campaign_core::symcert::record_seeds_at(&rec, &r.term).map_err(|x| {
                nx(
                    "catalogue_without_seeds",
                    format!("catalogue rule {}#{}: {x}", r.source, r.label),
                )
            })?;
        cat.push(rustred_campaign_core::symcert::CatalogueSeeds {
            source: r.source.clone(),
            label: r.label.clone(),
            record_sha256: r.record_sha256.clone(),
            term: r.term.clone(),
            seeds,
        });
    }
    let kind = e.verification["kind"]
        .as_str()
        .unwrap_or("two_row")
        .to_string();
    Ok(SymRelation {
        id: format!("{kind}#{}", e.index),
        producer: format!("repairflow:{kind}"),
        producer_record_sha256: Some(e.record_sha256.clone()),
        key: e.key.clone(),
        rhs: e.rhs.clone(),
        rows,
        catalogue: cat,
        group_digest: Some(e.group_digest.clone()),
        zero_sectors,
        own: json!({"verdict": "accepted", "by": "rr_rf_symflow two-row step + Symbolica second route (own algebra)",
            "verification": e.verification}),
    })
}

/// Step 4 (thread): the certificates of the round re-verified from their run directories; the round record.
fn round_pass(
    cdir: &Path,
    n: &str,
    node: &str,
    k: u32,
    routed: Value,
    runs: Vec<(KernelJob, Option<String>)>,
) -> Result<Vec<u8>, String> {
    let dir = round_dir(n, k);
    let table: Option<PinnedFile> = serde_json::from_value(routed["table"].clone()).ok();
    let ver: Option<PinnedFile> = serde_json::from_value(routed["verification"].clone()).ok();
    let ksha = routed["kernel"]["sha256"].as_str().map(str::to_string);
    let mut files = vec![];
    let mut no_lift: Vec<Value> = vec![];
    let mut findings: Vec<Value> = vec![];
    if let Some(sha) = &ksha {
        // the family the certificates must be bound to: from the first kernel job's request
        let verdicts: Vec<super::symcert::RelationVerdict> = runs
            .iter()
            .map(|(kj, rd)| super::symcert::RelationVerdict {
                id: kj.id.clone(),
                producer: kj.producer.clone(),
                producer_record_sha256: None,
                key: kj.key.clone(),
                relation_digest: None,
                kernel_sha256: sha.clone(),
                verdict: if rd.is_some() { "certified" } else { "failed" }.into(),
                detail: String::new(),
                certificate: None,
                certificate_sha256: None,
                job_id: None,
                run_dir: rd.clone(),
                row: None,
                attempts: 0,
                seconds: 0.0,
                finding: false,
            })
            .collect();
        let fam_b3 = runs
            .first()
            .and_then(|(kj, _)| std::fs::read(cdir.join(&kj.dir).join("request.json")).ok())
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .and_then(|v| v["family_blake3"].as_str().map(str::to_string))
            .unwrap_or_default();
        let binding = super::symcert::ImportBinding {
            kernels: vec![sha.clone()],
            family_blake3: fam_b3,
            roots: vec![cdir.to_path_buf()],
        };
        let (fs, no) = super::symcert::kernel_certificate_files(&verdicts, &binding);
        for f in fs {
            files.push(rustred_campaign_core::import::spec::FileRef {
                path: f.path,
                sha256: Some(f.sha256),
                manifest: None,
                family: None,
            });
        }
        for x in no {
            let finding = x.reason == "not_decided" && x.detail == "kernel_refused";
            let v = json!({"id": x.id, "key": x.key, "reason": x.reason, "detail": x.detail});
            if finding {
                findings.push(v.clone());
            }
            no_lift.push(v);
        }
    }
    let ci = match (table, ver) {
        (Some(table), Some(verification)) => Some(CompositeInput {
            table,
            verification,
            kernel_certificates: if files.is_empty() {
                None
            } else {
                Some(CertificateSpec {
                    files: files.clone(),
                    kernels: ksha.clone().into_iter().collect(),
                })
            },
        }),
        _ => None,
    };
    if !findings.is_empty() {
        // FINDINGS (both sides: the relation as our algebra accepted it = the routed record; the kernel's answer =
        // the sealed certificate in the run directory)
        let mut b = Vec::new();
        for f in &findings {
            b.extend_from_slice(f.to_string().as_bytes());
            b.push(b'\n');
        }
        let p = cdir.join(format!("{dir}/findings.jsonl"));
        crate::atomic::write_atomic(&p, &b).map_err(|e| format!("{}: {e}", p.display()))?;
    }
    let d = json!({"schema": ROUND_SCHEMA, "node": node, "round": k, "composite_input": ci,
        "accepted": routed["accepted"], "kernel_only": routed["kernel_only"],
        "kernel_route": routed["kernel"]["route"], "kernel_jobs": runs.len(),
        "certified": files.len(), "no_lift": no_lift, "findings": findings.len()});
    Ok(json_bytes(&d))
}
