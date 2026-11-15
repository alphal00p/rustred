//! Adapter of kind `symmetry-composite` (lane symjob; interfaces/symmetry-composite-job.md): the repair flow's symmetry
//! COMPOSITE step run by `rr_rf_symflow` (crate rustred-campaign-algebra, the only campaign crate that links Symbolica)
//! as a LEGACY child under the job contract, launched through the existing launch path (guard, marker, namespace).
//!
//! - [`SymCompRequest`] (node, import spec + base, per-face key lists, policy) -> the job's files ([`job_files`]: the
//!   keys file and the tool's request, every input pinned by SHA-256) -> a `JobRequest` for the supervisor's own launch
//!   path ([`job_request`]) or one attempt through [`SymmetryCompositeAdapter`] (`JobAdapter`).
//! - [`wrap`]: the contract's wrapper (`classify::symcomp::wrap`) with campaign-core's SHA-256; called by
//!   `adapters::legacy::wrap` at the supervisor's settle and by the adapter's `finish`.
//! - SAME-BACKEND RE-CHECK ([`load_import`], [`verify_relations`]): the shipped relations are CANDIDATES (accept mode
//!   `supplied_relations`); each one is kept ONLY when `repairflow::composite_verify::verify_line` passes against the
//!   pinned import (image re-derived, every replaced term's FIRST MATCH = the cited record_sha256, composition repeated
//!   mod p at the two engine primes x three values of d DIFFERENT from the child's, shipped terms compared as a map,
//!   every shipped term lower by the import's descent test). Lines are bound to the sealed results by SHA-256. This is
//!   the COMPOSER'S OWN BACKEND (rows_by_element / image_of / compose_image; lane symfix, audit r2-sym F3): a filter,
//!   never evidence above `imported`. EVIDENCE comes only from the node import, which re-verifies every supplied relation
//!   AT LOAD (interfaces/import.md 13.3): same backend -> `imported`, a checked kernel certificate (the independent
//!   SECOND ROUTE, [`composite_second_route`]) -> `per-key`. The ledger refuses a direct credit from this kind (F5).
//! - [`route_settled`]: the WorkSource-level function: a settled attempt (canonical lines of the verified results) ->
//!   [`SymCompRouted`] (per-key outcomes, accepted / refused relations, the accepted `supplied_relations` table +
//!   verification as content-addressed records, and the `route0.composite_input` value for the repair flow).
//!
//! Generic (L8): no node, arity, count, path or threshold is known here; everything is request / spec / policy data.

use super::job::{
    AdapterError, Ended, JobAdapter, JobPlan, Prepared, Program, classify_attempt, create_run,
    sha256_hex, write_spec,
};
use crate::state::Rung;
use crate::supervisor::worksource::{AcceptedRecord, JobRequest, Settled};
use rustred_campaign_contract::classify::symcomp::{self, SymClass, SymResults};
use rustred_campaign_contract::classify::{self, Accept, Classification, WaitStatus};
use rustred_campaign_contract::fake::FakeScript;
use rustred_campaign_contract::layout::RunLayout;
use rustred_campaign_contract::spec::{
    Budgets, Determinism, InputRef, JobKind, JobSpec, OnCaseBudget, OutputOptions, ProgramRef,
    SeedSpec,
};
use rustred_campaign_core::import::load::ImportedSet;
use rustred_campaign_core::import::{TableSetSpec, lines::parse_key_text};
use rustred_campaign_core::repairflow::composite_verify::{LineOptions, verify_line};
use rustred_campaign_core::repairflow::policy::{
    CompositeInput, PinnedFile, PolicyFile, RepairPolicy,
};
use rustred_campaign_core::replay_base::symrows::VerifiedGroup;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::io::{BufRead, Read};
use std::path::{Path, PathBuf};

/// The rung tag of symmetry-composite job ids.
pub const RUNG: &str = "sc";
/// Schema of a per-key outcome line ([`KeyRecord`]).
pub const KEY_SCHEMA: &str = "rustred.campaign.symmetry-composite-key.v1";
/// The values of d of the supervisor's modular re-check: DIFFERENT from the child's own independent check (the tool
/// uses 1000003, 7777771, 123456791), so the two checks share no evaluation point. Data: [`VerifyOptions::d_values`].
pub const DEFAULT_D_VALUES: [u64; 3] =
    rustred_campaign_core::repairflow::composite_verify::DEFAULT_D_VALUES;

/// One repair-flow request: the open keys of a node grouped by face (the flow's per-face key lists) and its policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymCompRequest {
    pub node: Option<String>,
    /// The node's import spec (absolute path).
    pub import_spec: PathBuf,
    /// The base the import spec's relative paths resolve against.
    pub import_base: PathBuf,
    /// (face label, keys) in the flow's order; the keys file lists them in this order, first occurrence kept.
    pub faces: Vec<(String, Vec<Vec<i64>>)>,
    /// The repair-flow policy file (`rustred.campaign.repairflow.v1`), if any.
    pub policy: Option<PathBuf>,
    pub threads: u32,
    pub limit: Option<u64>,
}

impl SymCompRequest {
    /// The keys in face order, duplicates dropped (first occurrence kept).
    pub fn keys(&self) -> Vec<Vec<i64>> {
        let mut seen = BTreeSet::new();
        self.faces
            .iter()
            .flat_map(|(_, ks)| ks.iter())
            .filter(|k| seen.insert((*k).clone()))
            .cloned()
            .collect()
    }
    /// The keys file (one JSON array per line).
    pub fn keys_bytes(&self) -> Vec<u8> {
        let mut b = Vec::new();
        for k in self.keys() {
            b.extend_from_slice(serde_json::to_string(&k).expect("json").as_bytes());
            b.push(b'\n');
        }
        b
    }
}

/// The files of one job and the spec inputs naming them (absolute paths).
#[derive(Debug, Clone)]
pub struct JobFiles {
    pub request: PathBuf,
    pub keys: PathBuf,
    pub inputs: Vec<InputRef>,
}

fn read_input(p: &Path) -> Result<Vec<u8>, AdapterError> {
    std::fs::read(p).map_err(|e| AdapterError::Io(format!("{}: {e}", p.display())))
}

fn input_ref(role: &str, p: &Path, b: &[u8]) -> InputRef {
    InputRef {
        role: role.into(),
        path: p.display().to_string(),
        blake3: rustred_campaign_contract::blake3_hex(b),
        upto: None,
    }
}

/// Write `keys.jsonl` and `request.json` (the tool's request; every input pinned by SHA-256, plus the program's
/// SHA-256) into `dir` (created), and return them with the spec inputs (roles request, import_spec, keys, policy).
/// Refuses an empty key list (nothing to ask) and existing files (never clobbered).
pub fn job_files(
    dir: &Path,
    req: &SymCompRequest,
    program_sha256: &str,
) -> Result<JobFiles, AdapterError> {
    let keys_b = req.keys_bytes();
    if keys_b.is_empty() {
        return Err(AdapterError::Request("no key".into()));
    }
    std::fs::create_dir_all(dir)
        .map_err(|e| AdapterError::Io(format!("{}: {e}", dir.display())))?;
    let dir = std::path::absolute(dir).map_err(|e| AdapterError::Io(e.to_string()))?;
    let spec_p =
        std::path::absolute(&req.import_spec).map_err(|e| AdapterError::Io(e.to_string()))?;
    let spec_b = read_input(&spec_p)?;
    let keys_p = dir.join("keys.jsonl");
    crate::atomic::create_new_atomic(&keys_p, &keys_b)
        .map_err(|e| AdapterError::Io(format!("{}: {e}", keys_p.display())))?;
    let mut inputs = vec![
        input_ref("import_spec", &spec_p, &spec_b),
        input_ref("keys", &keys_p, &keys_b),
    ];
    let policy = match &req.policy {
        Some(p) => {
            let p = std::path::absolute(p).map_err(|e| AdapterError::Io(e.to_string()))?;
            let b = read_input(&p)?;
            inputs.push(input_ref("policy", &p, &b));
            json!({"path": p.display().to_string(), "sha256": sha256_hex(&b)})
        }
        None => Value::Null,
    };
    let request = json!({"schema": symcomp::REQUEST_SCHEMA, "node": req.node,
        "import_spec": {"path": spec_p.display().to_string(), "sha256": sha256_hex(&spec_b)},
        "import_base": req.import_base.display().to_string(),
        "keys": {"path": keys_p.display().to_string(), "sha256": sha256_hex(&keys_b)},
        "policy": policy, "threads": req.threads.max(1), "limit": req.limit,
        symcomp::PROGRAM_SHA256: program_sha256});
    let rb = serde_json::to_vec_pretty(&request).expect("json");
    let req_p = dir.join("request.json");
    crate::atomic::create_new_atomic(&req_p, &rb)
        .map_err(|e| AdapterError::Io(format!("{}: {e}", req_p.display())))?;
    inputs.insert(0, input_ref("request", &req_p, &rb));
    Ok(JobFiles {
        request: req_p,
        keys: keys_p,
        inputs,
    })
}

/// The wrapper of the kind (the contract's, with campaign-core's SHA-256 and the licence's presence in this process).
pub fn wrap(layout: &RunLayout, spec: &JobSpec, spec_blake3: &str, wait: WaitStatus) -> WaitStatus {
    symcomp::wrap(
        layout,
        spec,
        spec_blake3,
        wait,
        &sha256_hex,
        std::env::var_os(symcomp::LICENCE_ENV).is_some(),
    )
}

// ------------------------------------------------------------------------------------------------ import verification

/// Options of the supervisor's verification (data).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifyOptions {
    pub d_values: Vec<u64>,
}

impl Default for VerifyOptions {
    fn default() -> Self {
        VerifyOptions {
            d_values: DEFAULT_D_VALUES.to_vec(),
        }
    }
}

/// The node's import as the job pinned it: the loaded set, the verified group, the policy.
pub struct ImportCtx {
    pub set: ImportedSet,
    pub group: VerifiedGroup,
    pub group_digest: String,
    pub policy: RepairPolicy,
    pub lines: Vec<bool>,
}

fn pinned_bytes(spec: &JobSpec, role: &str) -> Result<Option<Vec<u8>>, String> {
    let Some(i) = spec.inputs.iter().find(|i| i.role == role) else {
        return Ok(None);
    };
    let b = std::fs::read(spec.input_path(i)).map_err(|e| format!("input {role}: {e}"))?;
    if rustred_campaign_contract::blake3_hex(&b) != i.blake3 {
        return Err(format!(
            "input {role} changed (BLAKE3 differs from the spec's)"
        ));
    }
    Ok(Some(b))
}

/// Load the job's pinned import (spec input `import_spec`, base and node from the request, policy input), and derive
/// the verified group exactly as the tool does (policy `route0.family`, else the frame header's family).
pub fn load_import(spec: &JobSpec) -> Result<ImportCtx, String> {
    let req: Value =
        serde_json::from_slice(&pinned_bytes(spec, "request")?.ok_or("no request input")?)
            .map_err(|e| format!("request: {e}"))?;
    let node = req["node"].as_str();
    let base = PathBuf::from(req["import_base"].as_str().unwrap_or("/"));
    let policy = match pinned_bytes(spec, "policy")? {
        Some(b) => {
            PolicyFile::parse(&String::from_utf8_lossy(&b)).and_then(|f| f.effective(node))?
        }
        None => RepairPolicy::default(),
    };
    let sb = pinned_bytes(spec, "import_spec")?.ok_or("no import_spec input")?;
    let ts = TableSetSpec::from_json(&String::from_utf8_lossy(&sb))
        .map_err(|e| format!("import spec: {e}"))?;
    let set =
        rustred_campaign_core::import::load(&ts, &base).map_err(|e| format!("import load: {e}"))?;
    let lines = set.frame.mask().to_vec();
    let fsrc = ts
        .sources
        .iter()
        .find(|s| s.id == ts.frame_from)
        .ok_or("import spec: frame_from")?;
    let fpath = PathBuf::from(fsrc.path.clone().ok_or("frame_from path")?);
    let fpath = if fpath.is_absolute() {
        fpath
    } else {
        base.join(fpath)
    };
    let header: Value = {
        let f = std::fs::File::open(&fpath).map_err(|e| format!("{}: {e}", fpath.display()))?;
        let mut l = String::new();
        std::io::BufReader::new(f)
            .read_line(&mut l)
            .map_err(|e| e.to_string())?;
        serde_json::from_str(l.trim_end()).map_err(|e| format!("frame header: {e}"))?
    };
    let gs: rustred_campaign_core::import::symmetry::GroupSpec = match &policy.route0.family {
        Some(f) => serde_json::from_value(json!({"family": {"path": f}})),
        None => serde_json::from_value(json!({})),
    }
    .map_err(|e| e.to_string())?;
    let (group, rep) = rustred_campaign_core::import::symmetry::load_verified_group(
        &gs,
        &base,
        &lines,
        set.cmp.presentation().new_to_old(),
        header["family_source"].as_str(),
    )
    .map_err(|e| format!("verified group: {e}"))?;
    Ok(ImportCtx {
        set,
        group,
        group_digest: rep.digest,
        policy,
        lines,
    })
}

/// One relation the import's verification accepted: its key and its child lines (verbatim, bound by SHA-256).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedRelation {
    pub index: u64,
    pub key: Vec<i64>,
    pub plain: bool,
    pub table_line: Vec<u8>,
    pub verification_line: Vec<u8>,
}

/// The verdicts of the import's verification over a job's shipped relations.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Verified {
    pub accepted: Vec<AcceptedRelation>,
    /// (relation index, key, why) of every refused relation.
    pub refused: Vec<(u64, Vec<i64>, String)>,
    /// The child's table header line (copied into the accepted table).
    pub header: Vec<u8>,
}

fn file_lines(p: &Path) -> Result<(Vec<u8>, Vec<Vec<u8>>), String> {
    let mut b = Vec::new();
    std::fs::File::open(p)
        .and_then(|mut f| f.read_to_end(&mut b))
        .map_err(|e| format!("{}: {e}", p.display()))?;
    let lines = b
        .split(|&c| c == b'\n')
        .filter(|l| !l.is_empty())
        .map(<[u8]>::to_vec)
        .collect();
    Ok((b, lines))
}

/// THE IMPORT'S VERIFICATION of every shipped relation of a settled attempt (module documentation). `run_root` = the
/// attempt's run directory; `results` = the sealed results re-read by the contract ([`symcomp::tally`]). A child file
/// whose SHA-256 is not the sealed one refuses every relation; a relation whose lines are not the bound ones, whose
/// group is not the import's, or which fails `verify_shipped`, is refused with the reason.
pub fn verify_relations(
    run_root: &Path,
    results: &SymResults,
    ctx: &ImportCtx,
    opts: &VerifyOptions,
) -> Verified {
    let dir = run_root.join("out").join(symcomp::CHILD_DIR);
    let mut out = Verified::default();
    let refuse_all = |out: &mut Verified, why: &str| {
        for r in &results.relations {
            out.refused
                .push((r.index, r.target.clone(), why.to_string()));
        }
    };
    let mut lines = std::collections::BTreeMap::new();
    for f in [symcomp::TABLE_FILE, symcomp::VERIFICATION_FILE] {
        match file_lines(&dir.join(f)) {
            Ok((b, ls))
                if results.files.get(f).map(|x| x.0.as_str()) == Some(sha256_hex(&b).as_str()) =>
            {
                lines.insert(f, ls);
            }
            Ok(_) => {
                refuse_all(&mut out, &format!("{f} changed after it was sealed"));
                return out;
            }
            Err(e) => {
                refuse_all(&mut out, &e);
                return out;
            }
        }
    }
    let table = &lines[symcomp::TABLE_FILE];
    let ver = &lines[symcomp::VERIFICATION_FILE];
    out.header = table.first().cloned().unwrap_or_default();
    let arity = ctx.lines.len();
    let lopts = LineOptions {
        d_values: opts.d_values.clone(),
        max_row_terms: ctx.policy.route0.max_terms,
        min_depth: ctx.policy.composite.max_depth,
    };
    for r in &results.relations {
        let j = r.index as usize;
        let check = || -> Result<(Vec<u8>, Vec<u8>), String> {
            let tl = table.get(j + 1).ok_or("no table line")?;
            let vl = ver.get(j).ok_or("no verification line")?;
            if sha256_hex(tl) != r.line_sha256 || sha256_hex(vl) != r.verification_sha256 {
                return Err("the child's lines are not the sealed ones".into());
            }
            if r.group_digest != ctx.group_digest {
                return Err(format!(
                    "group digest {} is not the import's verified group {}",
                    r.group_digest, ctx.group_digest
                ));
            }
            let tv: Value = serde_json::from_slice(tl).map_err(|e| e.to_string())?;
            let vv: Value = serde_json::from_slice(vl).map_err(|e| e.to_string())?;
            let key: Vec<i32> = r.target.iter().map(|&x| x as i32).collect();
            let kt = |t: &Value| -> Result<Vec<i32>, String> {
                t.as_str()
                    .and_then(|s| parse_key_text(s, arity))
                    .ok_or_else(|| format!("key text {t}"))
            };
            let rhs: Vec<(Vec<i32>, String)> = tv["rhs"]
                .as_array()
                .ok_or("rhs")?
                .iter()
                .map(|p| {
                    Ok((
                        kt(&p[0])?,
                        p[1].as_str().ok_or("coefficient text")?.to_string(),
                    ))
                })
                .collect::<Result<_, String>>()?;
            let cited: Vec<&str> = vv["replacements"]
                .as_array()
                .ok_or("replacements")?
                .iter()
                .map(|x| x["record_sha256"].as_str().unwrap_or_default())
                .collect();
            if cited.iter().copied().ne(r.records.iter().map(String::as_str)) {
                return Err("the cited records are not the sealed ones".into());
            }
            // the SAME-BACKEND re-check (interfaces/import.md 13.3 (c)): the composer's own functions, modular at d
            // values the child did not use; the one entry shared with the import's load-time verification
            verify_line(&ctx.set, &ctx.group, &key, &rhs, &vv, &lopts)?;
            Ok((tl.clone(), vl.clone()))
        };
        match check() {
            Ok((t, v)) => out.accepted.push(AcceptedRelation {
                index: r.index,
                key: r.target.clone(),
                plain: r.plain,
                table_line: t,
                verification_line: v,
            }),
            Err(why) => out.refused.push((r.index, r.target.clone(), why)),
        }
    }
    out
}

// ------------------------------------------------------------------------------------------------ outcomes

/// One key's outcome after the import's verification (JSONL, schema [`KEY_SCHEMA`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyRecord {
    pub schema: String,
    pub key: Vec<i64>,
    pub face: String,
    /// The child's class: plain | composite | none | over_cap | error.
    pub outcome: String,
    /// `accepted` (relation verified by the import) | `refused` (relation shipped, refused by the import) |
    /// `no_relation` (typed per-key outcome, `failure_class` + `route`).
    pub verdict: String,
    pub relation: Option<u64>,
    pub why: Option<String>,
    pub failure_class: Option<String>,
    pub route: Option<String>,
    pub status: String,
}

/// What a settled symmetry-composite attempt gives the work source.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SymCompRouted {
    pub per_key: Vec<KeyRecord>,
    pub verified: Verified,
    /// The accepted `supplied_relations` table (child header + accepted lines verbatim) and its verification lines.
    pub table: Vec<u8>,
    pub verification: Vec<u8>,
}

impl SymCompRouted {
    /// Counts per child class (`plain`, `composite`, `none`, `over_cap`, `error`) and verdicts.
    pub fn counts(&self) -> std::collections::BTreeMap<String, u64> {
        let mut m = std::collections::BTreeMap::new();
        for k in &self.per_key {
            *m.entry(k.outcome.clone()).or_default() += 1;
            *m.entry(format!("verdict:{}", k.verdict)).or_default() += 1;
        }
        m
    }
    /// The accepted table / verification / per-key files as content-addressed records under `dir` (CDIR-relative).
    pub fn records(&self, dir: &str) -> Vec<AcceptedRecord> {
        let mut keys = Vec::new();
        for k in &self.per_key {
            keys.extend_from_slice(serde_json::to_string(k).expect("json").as_bytes());
            keys.push(b'\n');
        }
        [
            ("table.jsonl", &self.table),
            ("verification.jsonl", &self.verification),
            ("keys.jsonl", &keys),
        ]
        .into_iter()
        .map(|(n, b)| AcceptedRecord {
            path: format!(
                "{dir}/symcomp.{}.{n}",
                &rustred_campaign_contract::blake3_hex(b)[..16]
            ),
            bytes: b.clone(),
        })
        .collect()
    }
    /// The repair flow's `route0.composite_input` naming the accepted table / verification records (absolute paths
    /// under `cdir`, SHA-256 pinned); `None` when nothing was accepted.
    pub fn composite_input(&self, cdir: &Path, dir: &str) -> Option<CompositeInput> {
        if self.verified.accepted.is_empty() {
            return None;
        }
        let r = self.records(dir);
        let pin = |a: &AcceptedRecord| PinnedFile {
            path: cdir.join(&a.path).display().to_string(),
            sha256: sha256_hex(&a.bytes),
        };
        Some(CompositeInput {
            table: pin(&r[0]),
            verification: pin(&r[1]),
            kernel_certificates: None,
        })
    }
}

/// Per-key records + accepted files from the contract's re-read results and the import's verdicts.
pub fn route_results(results: &SymResults, verified: Verified) -> SymCompRouted {
    let acc: std::collections::BTreeMap<u64, &AcceptedRelation> =
        verified.accepted.iter().map(|a| (a.index, a)).collect();
    let refd: std::collections::BTreeMap<u64, &str> = verified
        .refused
        .iter()
        .map(|(i, _, w)| (*i, w.as_str()))
        .collect();
    let per_key = results
        .keys
        .iter()
        .map(|k| {
            let (verdict, why) = match k.relation {
                Some(j) if acc.contains_key(&j) => ("accepted", None),
                Some(j) => (
                    "refused",
                    Some(refd.get(&j).copied().unwrap_or("not verified").to_string()),
                ),
                None => ("no_relation", None),
            };
            let fc = k.failure_class();
            KeyRecord {
                schema: KEY_SCHEMA.into(),
                key: k.key.clone(),
                face: k.face.clone(),
                outcome: k.class.word().into(),
                verdict: verdict.into(),
                relation: k.relation,
                why,
                failure_class: fc.map(|c| c.name()),
                route: fc.map(|c| {
                    serde_json::to_value(c.route())
                        .ok()
                        .and_then(|v| v.as_str().map(str::to_string))
                        .unwrap_or_default()
                }),
                status: k.status.clone(),
            }
        })
        .collect();
    let mut table = Vec::new();
    let mut ver = Vec::new();
    if !verified.accepted.is_empty() {
        table.extend_from_slice(&verified.header);
        table.push(b'\n');
        for a in &verified.accepted {
            table.extend_from_slice(&a.table_line);
            table.push(b'\n');
            ver.extend_from_slice(&a.verification_line);
            ver.push(b'\n');
        }
    }
    SymCompRouted {
        per_key,
        verified,
        table,
        verification: ver,
    }
}

/// Read the sealed results of a classified attempt (through the contract's verified handle) and route them:
/// relations verified by the import when the row accepts `supplied_relations`; per-key outcomes for row 35.
fn routed_of(
    run_root: &Path,
    spec: &JobSpec,
    c: &Classification,
    lines: &[Vec<u8>],
    import: Option<&ImportCtx>,
    opts: &VerifyOptions,
) -> Result<Option<SymCompRouted>, String> {
    if !(c.accept == Accept::SuppliedRelations || c.row == 35) {
        return Ok(None);
    }
    let (res, _) = symcomp::tally(lines, spec)?;
    let verified = if c.accept == Accept::SuppliedRelations && !res.relations.is_empty() {
        let owned;
        let ctx = match import {
            Some(x) => x,
            None => {
                owned = load_import(spec)?;
                &owned
            }
        };
        verify_relations(run_root, &res, ctx, opts)
    } else {
        Verified::default()
    };
    Ok(Some(route_results(&res, verified)))
}

// ------------------------------------------------------------------------------------------------ the JobAdapter

/// The output of one attempt.
#[derive(Debug, Clone)]
pub struct SymCompBatch {
    pub job_id: String,
    pub classification: Classification,
    pub mapped_wait: WaitStatus,
    pub routed: Option<SymCompRouted>,
    /// Why nothing could be routed (the wrapper's refusal text, a results / import error).
    pub refusal: Option<String>,
}

/// Adapter of kind symmetry-composite.
pub struct SymmetryCompositeAdapter {
    /// `rr_rf_symflow` (invocation Legacy), or the fake child (invocation Contract, with `fake`).
    pub program: Program,
    /// The Symbolica licence variable's NAME (passed through by the launcher).
    pub licence_env: Option<String>,
    pub verify: VerifyOptions,
    pub fake: Option<FakeScript>,
}

fn program_sha(p: &Program) -> Result<String, AdapterError> {
    match &p.sha256 {
        Some(s) => Ok(s.clone()),
        None => Ok(sha256_hex(&read_input(&p.path)?)),
    }
}

/// The job spec of one attempt (legacy constraints: no resume, no budgets, sample seed 0).
pub fn job_spec(
    job_id: &str,
    attempt: u32,
    run_dir: &Path,
    program: &Program,
    inputs: Vec<InputRef>,
    fake: Option<FakeScript>,
) -> JobSpec {
    JobSpec {
        schema: rustred_campaign_contract::CONTRACT.into(),
        job_id: job_id.into(),
        attempt,
        kind: JobKind::SymmetryComposite,
        run_dir: run_dir.display().to_string(),
        program: ProgramRef {
            name: program.name.clone(),
            blake3: program.blake3.clone(),
            features_required: program.features_required.clone(),
        },
        inputs,
        args: vec![],
        seed: SeedSpec::default(),
        budgets: Budgets::default(),
        on_case_budget: OnCaseBudget::Stop,
        determinism: Determinism::Recorded,
        output: OutputOptions::default(),
        heartbeat_s: 10,
        stop_grace_s: 5,
        resume_every_s: 0,
        resume: None,
        env_allow: vec![],
        fake,
    }
}

impl JobAdapter for SymmetryCompositeAdapter {
    type Request = SymCompRequest;
    type Output = SymCompBatch;

    fn kind(&self) -> JobKind {
        JobKind::SymmetryComposite
    }

    fn plan(&self, reqs: &[SymCompRequest]) -> Result<Vec<JobPlan>, AdapterError> {
        reqs.iter()
            .enumerate()
            .map(|(i, r)| {
                if r.keys().is_empty() {
                    return Err(AdapterError::Request(format!("request {i}: no key")));
                }
                let spec_sha = sha256_hex(&read_input(&r.import_spec)?);
                let pol = match &r.policy {
                    Some(p) => Some(sha256_hex(&read_input(p)?)),
                    None => None,
                };
                let id = json!({"kind": "symmetry-composite", "program": self.program.blake3, "node": r.node,
                    "import_spec": spec_sha, "base": r.import_base, "policy": pol, "limit": r.limit,
                    "keys": rustred_campaign_contract::blake3_hex(&r.keys_bytes())});
                let content =
                    rustred_campaign_contract::blake3_hex(id.to_string().as_bytes())[..16].to_string();
                Ok(JobPlan {
                    job_id: rustred_campaign_contract::layout::job_id(&content, RUNG),
                    members: vec![i],
                    content,
                })
            })
            .collect()
    }

    fn prepare(
        &self,
        reqs: &[SymCompRequest],
        plan: &JobPlan,
        work_root: &Path,
        attempt: u32,
    ) -> Result<Prepared, AdapterError> {
        let root = std::path::absolute(work_root).map_err(|e| AdapterError::Io(e.to_string()))?;
        let r = plan
            .members
            .first()
            .map(|&m| &reqs[m])
            .ok_or_else(|| AdapterError::Request("empty plan".into()))?;
        let layout = create_run(&root, &plan.job_id, attempt)?;
        let files = job_files(&layout.in_dir(), r, &program_sha(&self.program)?)?;
        let spec = job_spec(
            &plan.job_id,
            attempt,
            &layout.root,
            &self.program,
            files.inputs,
            self.fake.clone(),
        );
        let argv = symcomp::argv(&spec).map_err(AdapterError::Spec)?;
        write_spec(&spec, &self.program, self.licence_env.as_deref(), &argv)
    }

    fn finish(
        &self,
        _reqs: &[SymCompRequest],
        plan: &JobPlan,
        prep: &Prepared,
        end: &Ended,
    ) -> SymCompBatch {
        let mapped = wrap(&prep.layout, &prep.spec, &prep.spec_blake3, end.wait);
        let e2 = Ended {
            wait: mapped,
            kill: end.kill,
            oom: end.oom,
        };
        let (ev, c) = classify_attempt(prep, &e2);
        let mut out = SymCompBatch {
            job_id: plan.job_id.clone(),
            classification: c.clone(),
            mapped_wait: mapped,
            routed: None,
            refusal: std::fs::read_to_string(prep.layout.sup_dir().join(symcomp::REFUSAL_FILE))
                .ok(),
        };
        let lines = (|| -> Result<Vec<Vec<u8>>, String> {
            let v = ev.verified.as_ref().ok_or("no verified outputs")?;
            let rel = RunLayout::out_rel(rustred_campaign_contract::layout::RESULTS);
            let s = v
                .streams
                .iter()
                .find(|s| s.rel == rel)
                .ok_or("no results stream")?;
            rustred_campaign_contract::seal::VerifiedHandle::reopen(&prep.layout.root, s)
                .and_then(|h| h.canonical_lines())
                .map_err(|e| format!("results: {e}"))
        })();
        if c.accept == Accept::SuppliedRelations || c.row == 35 {
            match lines
                .and_then(|l| routed_of(&prep.layout.root, &prep.spec, &c, &l, None, &self.verify))
            {
                Ok(r) => out.routed = r,
                Err(e) => out.refusal = Some(e),
            }
        }
        out
    }
}

// ------------------------------------------------------------------------------------------------ WorkSource level

/// What a work source configures for its symmetry-composite jobs (data).
#[derive(Debug, Clone)]
pub struct SymCompJobConfig {
    /// Key of `spec.programs` (the supervisor resolves the path, BLAKE3 and invocation).
    pub program: String,
    pub cap_bytes: u64,
    pub expected_peak_bytes: u64,
    pub cores: usize,
    pub heartbeat_s: u32,
    pub priority: i64,
    pub fake: Option<FakeScript>,
}

/// A `JobRequest` of kind symmetry-composite for the supervisor's own launch path (guard, marker, namespace; the
/// supervisor derives the legacy argv from the spec through `adapters::legacy::argv` and wraps at settle through
/// `adapters::legacy::wrap`). `files` = [`job_files`] written by the source under its node directory (before
/// `ready()` returns them); `depends_on` = the ledger input ids of those files, if registered.
pub fn job_request(
    item: &str,
    job: &str,
    files: &JobFiles,
    cfg: &SymCompJobConfig,
    depends_on: Vec<String>,
) -> JobRequest {
    JobRequest {
        job: job.into(),
        item: item.into(),
        rung: Rung(0),
        kind: JobKind::SymmetryComposite,
        priority: cfg.priority,
        cap_bytes: cfg.cap_bytes,
        expected_peak_bytes: cfg.expected_peak_bytes,
        cores: cfg.cores.max(1),
        heartbeat_s: cfg.heartbeat_s.max(1),
        stop_grace_s: 5,
        program: cfg.program.clone(),
        args: vec![],
        inputs: files.inputs.clone(),
        depends_on,
        budgets: Budgets::default(),
        sample_seed: 0,
        fake: cfg.fake.clone(),
    }
}

/// THE WORKSOURCE-LEVEL FUNCTION: a settled symmetry-composite attempt -> [`SymCompRouted`] (or `None` when the row
/// routes nothing: refusal, crash, loss; the source then uses the classification's action). `cdir` = the campaign
/// directory `done.table` is relative to; the run directory is the results file's grand-parent; the spec is re-read
/// from it and must be the kind's; the canonical lines are the supervisor's verified ones (`done.canonical_lines`).
pub fn route_settled(
    cdir: &Path,
    done: &Settled,
    import: Option<&ImportCtx>,
    opts: &VerifyOptions,
) -> Result<Option<SymCompRouted>, String> {
    let c = &done.classification;
    if !(c.accept == Accept::SuppliedRelations || c.row == 35) {
        return Ok(None);
    }
    let t = done
        .table
        .as_ref()
        .ok_or("a settled attempt without its sealed results")?;
    let results = cdir.join(&t.path);
    let run_root = results
        .parent()
        .and_then(Path::parent)
        .ok_or("results path without a run directory")?;
    let spec_b =
        std::fs::read(RunLayout::new(run_root).job_spec()).map_err(|e| format!("job.json: {e}"))?;
    let spec: JobSpec = serde_json::from_slice(&spec_b).map_err(|e| format!("job.json: {e}"))?;
    if spec.kind != JobKind::SymmetryComposite
        || spec.job_id != done.job
        || spec.attempt != done.attempt
    {
        return Err("job.json is not this symmetry-composite attempt's".into());
    }
    let lines: Vec<Vec<u8>> = done
        .canonical_lines
        .iter()
        .map(|l| l.as_bytes().to_vec())
        .collect();
    routed_of(run_root, &spec, c, &lines, import, opts)
}

/// The classification classes of the kind (for logs): whether the row accepted candidates.
pub fn accepts_candidates(c: &Classification) -> bool {
    c.accept == Accept::SuppliedRelations && classify::ROWS.iter().any(|r| r.row == c.row)
}

/// Keys of `per_key` with a given child class (convenience for call sites and tests).
pub fn keys_of(r: &SymCompRouted, class: SymClass) -> Vec<Vec<i64>> {
    r.per_key
        .iter()
        .filter(|k| k.outcome == class.word())
        .map(|k| k.key.clone())
        .collect()
}

// ------------------------------------------------------------------------------------------------ second route (F3)

/// Feature names (an engine directory's MANIFEST `version.json`, its `features` list) that declare the kernel's
/// symmetry-row certificates (`rr_proof --mode instance-span --symrows`, contract kind proof-symrows). PROPOSED names (no
/// frozen manifest lists one yet: request in notes/implementer.symfix.md); the first one present decides.
pub const SYMROWS_FEATURES: [&str; 2] = ["proof_symrows", "symrows"];

/// The default second route of a composite (interfaces/import.md 13.3, closeloop.md 24).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompositeRoute {
    /// The kernel certificate (independent of the composer): the kernel program and its SHA-256 (the import's
    /// `kernel_certificates.kernels` allow-list entry).
    Kernel { program: PathBuf, sha256: String },
    /// No kernel available: the same-backend re-verification alone (level `imported`), with the reason.
    SameBackend { why: String },
}

/// Decide the default second route from the engine directory's MANIFEST (`version.json` features, as lane seedmin reads
/// capabilities) and its `SHA256SUMS` (the kernel binary `program` listed with the SHA-256 of the file as it is). NO
/// process is started (every child of the supervisor goes through the ONE launch path); a missing manifest, feature,
/// listing or a digest mismatch gives [`CompositeRoute::SameBackend`] with the reason, never a guess.
pub fn composite_second_route(engine_dir: &Path, program: &str) -> CompositeRoute {
    let same = |why: String| CompositeRoute::SameBackend { why };
    let manifest = engine_dir.join("version.json");
    let features: Vec<String> = match std::fs::read(&manifest)
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
    {
        Some(v) => v["features"]
            .as_array()
            .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
            .unwrap_or_default(),
        None => return same(format!("no readable manifest {}", manifest.display())),
    };
    if !features.iter().any(|f| SYMROWS_FEATURES.contains(&f.as_str())) {
        return same(format!(
            "manifest {} declares none of {SYMROWS_FEATURES:?}",
            manifest.display()
        ));
    }
    let sums = engine_dir.join("SHA256SUMS");
    let listed = std::fs::read_to_string(&sums).ok().and_then(|t| {
        t.lines().find_map(|l| {
            let mut it = l.split_whitespace();
            let (h, n) = (it.next()?, it.next()?);
            (n.trim_start_matches('*') == program).then(|| h.to_ascii_lowercase())
        })
    });
    let Some(want) = listed else {
        return same(format!("{} does not list {program}", sums.display()));
    };
    let path = engine_dir.join(program);
    match std::fs::read(&path) {
        Ok(b) if sha256_hex(&b) == want => CompositeRoute::Kernel {
            program: path,
            sha256: want,
        },
        Ok(_) => same(format!("{} differs from its SHA256SUMS entry", path.display())),
        Err(e) => same(format!("{}: {e}", path.display())),
    }
}
